# Type-safe query results in Python: state of the art (research for Aureline codegen)

> **Provenance:** Researched via web search and primary sources (project docs, GitHub READMEs, typing PEPs)
> by a Claude research agent on **2026-08-13**. Informs the design of Aureline's generated-Python target,
> where the Rust compiler infers each query's result type and emits concrete Python types — Python carries
> types only, with no runtime validation of its own.

## 1. How existing tools do it

### sqlc-gen-python (SQL → Python codegen)
[sqlc-gen-python](https://github.com/sqlc-dev/sqlc-gen-python) is the closest analogue to Aureline: a compiler
that knows the result shape ahead of time and emits per-query Python. Its choices:

- **Rows are plain `@dataclasses.dataclass` classes by default**; `emit_pydantic_models: true` switches to
  `pydantic.BaseModel` for users who want validation ([README](https://github.com/sqlc-dev/sqlc-gen-python/blob/main/README.md)).
- **Enums** become `class Status(str, enum.Enum)` (or `enum.StrEnum` with `emit_str_enum` on 3.11+), so values
  compare equal to strings while staying nominal for the checker.
- **Sync and async queriers are separate emit options** (`emit_sync_querier` / `emit_async_querier`); `:many`
  queries return iterators/async iterators of the row class, `:one` returns `Optional[Row]`.

### EdgeDB / Gel Python codegen
[Gel's codegen](https://docs.geldata.com/reference/using/python/api/codegen) (`gel-py`, formerly `edgedb-py`)
generates **one typed function per `.edgeql` file** ([blog](https://www.geldata.com/blog/typesafe-database-querying-via-code-generation)):

- Signature pattern: `async def get_number(client: gel.AsyncIOClient, *, arg: int) -> int:` — query variables
  become keyword-only parameters; the return annotation is the inferred result type.
- Cardinality maps directly: single → `T`, optional → `Optional[T]`, multi → `list[T]`. Object shapes are
  **dataclasses** named after the query (e.g. `GetUserResult`).
- `--target {async|blocking|pydantic}` — async is the default; Pydantic is opt-in, mirroring sqlc. Community
  generators ([edgedb-pydantic-codegen](https://pypi.org/project/edgedb-pydantic-codegen/),
  [gel-pydantic-codegen](https://github.com/Japan7/gel-pydantic-codegen)) exist for FastAPI users, confirming
  the ecosystem expectation: *dataclasses by default, Pydantic as an integration option*.

### Prisma Client Python
[Prisma Client Python](https://github.com/RobertCraigie/prisma-client-py) renders Jinja2 templates into
**Pydantic models for every schema model**; all query methods return those models and are
"fully statically typed" ([docs](https://prisma-client-py.readthedocs.io/en/stable/getting_started/type-safety/)).
Partial selections get dedicated generated "partial types". Trade-off: heavy import cost, Pydantic as a hard
dependency, and redundant runtime validation of data the DB already guarantees — a known pain point.

### SQLAlchemy 2.0 typed mode
SQLAlchemy 2.0 threads `Mapped[int]` annotations through `select()` → `Result` → `Row`
([What's New](https://docs.sqlalchemy.org/en/20/changelog/whatsnew_20.html)), but hits the limits of doing this
*inside* the type system rather than via codegen: `Row` supports only positional tuple typing via a `.t`
accessor, `row.name` attribute access resolves to `Any`, and pyright inference is fragile
([#10487](https://github.com/sqlalchemy/sqlalchemy/discussions/10487),
[#11475](https://github.com/sqlalchemy/sqlalchemy/issues/11475)). Lesson: **ahead-of-time codegen (Aureline's
approach) sidesteps an entire class of generic-inference bugs** that plague in-language typed query builders.

### Official surrealdb Python SDK (the baseline Aureline improves on)
[surrealdb.py](https://github.com/surrealdb/surrealdb.py): `.query()` returns an untyped `Value` — in practice
`dict[str, Any]` / `list[dict]` (a tuple of values for multi-statement queries)
([executing queries](https://surrealdb.com/docs/languages/python/concepts/executing-queries)). There is **no
per-query typing at all today**. Rich runtime types do exist and Aureline's generated code should reuse them:
`RecordID` (with `.table_name`/`.id`, str-convertible), `Table`, `Duration`, `Range`, geometry classes;
`decimal.Decimal`, `datetime`, and `uuid.UUID` round-trip via CBOR
([data types](https://surrealdb.com/docs/sdk/python/data-types),
[releases](https://github.com/surrealdb/surrealdb.py/releases) note Duration/Decimal codec fixes).

Live queries: `db.live(table) -> UUID`, then `db.subscribe_live(uuid)` returns a **`Generator` (sync) or
`AsyncGenerator` (async)** of notification dicts `{"action": "CREATE"|"UPDATE"|"DELETE", "result": ...}`;
`db.kill(uuid)` stops the stream ([live queries](https://surrealdb.com/docs/sdk/python/concepts/live-queries)).
WebSocket connections only.

## 2. Typing features and carrier trade-offs (2026)

| Carrier | Runtime cost | Strictness | Fit for compile-time-only codegen |
|---|---|---|---|
| `TypedDict` | zero (it *is* a dict) | structural; no attribute access; typos in keys caught, but values are unchecked dicts at runtime | good for wire-shaped data; `Required`/`NotRequired` (PEP 655) and `ReadOnly` (PEP 705) help |
| `@dataclass(frozen=True, slots=True)` | one cheap constructor call per row; no deps | nominal; attribute access; `__slots__` ≈ msgspec memory profile | **best default** — matches sqlc + Gel convention |
| Pydantic v2 `BaseModel` | validation on every construction; import weight | nominal + coercion | redundant: the SDK's CBOR codec already produces correct Python types; keep as opt-in only |
| `msgspec.Struct` | fastest decode (2–5x pydantic v2, [benchmarks](https://gist.github.com/jcrist/d62f450594164d284fbea957fd48b743)) | nominal, tagged-union support | adds a hard third-party dep; wins only if Aureline owned deserialization, which it doesn't |

Key features to use:

- **`Literal` unions** for checked-enum fields: `status: Literal["draft", "published"]`. Zero runtime cost,
  perfect narrowing in pyright/mypy. (An `enum.StrEnum` alternative costs a runtime lookup and forces imports;
  Literal is the better fit when Aureline's compiler already guarantees the value set.)
- **Tagged unions via `Literal` discriminators** for live notifications — exactly because DELETE carries only
  the record id:

  ```python
  @dataclass(frozen=True, slots=True)
  class ProductCreated:
      action: Literal["CREATE"]
      result: Product

  @dataclass(frozen=True, slots=True)
  class ProductDeleted:
      action: Literal["DELETE"]
      result: RecordId[Product]  # id only

  ProductNotification = ProductCreated | ProductUpdated | ProductDeleted
  ```

  All major checkers narrow on `if n.action == "DELETE":` for both dataclass and TypedDict unions.
- **Branded record ids**: `NewType` cannot be generic, so use a tiny generic wrapper
  `class RecordId(Generic[T])` (or PEP 695 `class RecordId[T]:`) over the SDK's `RecordID`, phantom-typed by
  table: `RecordId[Product]`. This gives cross-table id confusion errors at check time with no runtime cost
  beyond the SDK object that exists anyway.
- **PEP 695 syntax (`class Row[T]`, `type X = ...`)** is 3.12+ *grammar* — there is **no
  `from __future__` backport** ([typeshed tracker](https://github.com/python/typeshed/issues/10869)). Pyright
  supported it at 3.12 release; mypy's support landed incrementally
  ([mypy #17233](https://github.com/python/mypy/pull/17233)) and is on by default in current releases.
  Generated code targeting ≥3.10 should stick to `Generic[T]` + `from __future__ import annotations`.

## 3. Type checkers that matter in 2026

- **Pyright** — 97.8% typing-spec conformance (March 2026), the strictness bar; test generated code under
  `pyright --strict` ([conformance comparison](https://www.danilchenko.dev/posts/pyrefly-vs-mypy-vs-ty/)).
- **mypy** — still the CI default in many shops despite 58.3% conformance; generated code must avoid
  mypy-fragile patterns (exotic overloads, self-type tricks).
- **Pyrefly (Meta, Rust, 1.0 in May 2026)** — 87.8% conformance, 10–50x faster; increasingly used in CI.
- **ty (Astral)** — fastest engine but ~53% conformance, still beta; treat as editor-feedback tier
  ([comparison](https://blog.edward-li.com/tech/comparing-pyrefly-vs-ty/)).

Compatibility checklist for generated packages: ship a **`py.typed` marker** (PEP 561) in the generated
package; use `from __future__ import annotations` for forward refs and older-Python syntax (safe because
dataclass field *types* are resolved lazily and nothing introspects annotations at runtime); avoid
`TypeAlias`-heavy indirection that ty/pyrefly still mishandle; keep every public symbol explicitly annotated
so inference differences between checkers never surface.

## 4. Async streams and paginators

- Gel/EdgeDB and Convex-style clients expose **plain `AsyncIterator` returns consumed with `async for`** —
  no callback registration. surrealdb.py's `subscribe_live` already returns an `AsyncGenerator`, so Aureline
  should wrap it as `AsyncIterator[ProductNotification]` with the typed tagged union as the element type.
- Recommended shape: a small generated handle
  `class ProductLiveQuery: def __aiter__(self) -> AsyncIterator[ProductNotification]; async def kill(self) -> None`,
  plus `async with` support so `kill()` is automatic — mirroring how the SDK pairs `live()`/`kill()`
  ([streaming docs](https://surrealdb.com/docs/sdk/python/concepts/streaming)).
- Paginated `:many` queries: return `AsyncIterator[Row]` directly (sqlc's async `:many` pattern); offer
  `async def all(self) -> list[Row]` as a convenience. Do not invent a bespoke cursor protocol.

## 5. Scalar mappings (SurrealQL → generated annotation)

| SurrealQL | Python annotation | Notes |
|---|---|---|
| `string` | `str` | |
| `int` / `float` | `int` / `float` | |
| `decimal` | `decimal.Decimal` | SDK CBOR codec handles it |
| `datetime` | `datetime.datetime` | tz-aware; SurrealDB is ns-precision, Python µs — document the truncation |
| `duration` | `surrealdb.Duration` | do **not** map to `timedelta` (loses ns); re-export from generated pkg |
| `uuid` | `uuid.UUID` | |
| `bytes` | `bytes` | |
| `record<product>` | `RecordId[Product]` | branded wrapper over SDK `RecordID` |
| `option<T>` | `T | None` | |
| enum-like unions | `Literal[...]` | |

## 6. Recommendation for Aureline

**Generate frozen, slotted dataclasses per query result shape** (`@dataclass(frozen=True, slots=True)`), one
typed function per query in the Gel style (`async def get_products(db: AsyncSurreal, *, min_price: Decimal) -> list[GetProductsRow]`),
with `Literal` unions for enums, `Literal`-discriminated dataclass unions for live notifications, a generic
`RecordId[T]` brand, `AsyncIterator` streams, and a `py.typed` package validated in CI against **pyright
strict + mypy + pyrefly**. Offer Pydantic emission later as an opt-in flag (the sqlc/Gel precedent), never as
the default.

**Avoid:** Pydantic-by-default (runtime cost with zero benefit under Aureline's compile-time guarantees, and
a heavy dependency); msgspec (dep for no gain since the SDK owns decoding); raw `TypedDict` as the primary
carrier (no attribute access, and dict-typos survive at runtime — acceptable only as an alternate "zero-copy"
emission mode); PEP 695 syntax unless the floor is 3.12+; SQLAlchemy-style in-type-system tuple inference
(`Row.t`-style accessors) — codegen makes it unnecessary and it is exactly where checkers disagree.
