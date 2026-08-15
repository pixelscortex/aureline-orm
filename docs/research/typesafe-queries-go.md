# Type-Safe Query Results in Go — Feasibility for Aureline Codegen

**Provenance.** Researched 2026-08-13 against primary sources: Go release notes
([1.24](https://go.dev/doc/go1.24), [1.25](https://go.dev/doc/go1.25),
[1.26](https://go.dev/doc/go1.26)), open golang/go proposals, sqlc docs, and the official
`surrealdb.go` SDK (v1.6.0, published 2026-07-14). Context: Aureline's Rust compiler infers each
query's result type ahead of time and *generates* concrete Go types per query — Go never has to
infer anything, only carry the types. The question is how far Go can carry them.

**Bottom line.** Go can reach roughly the sqlc bar — which is high and idiomatic — but not the
TS/Rust bar. Everything nominal works: per-query row structs, branded record IDs, typed enums,
generic result plumbing. What cannot be expressed is *closed-ness*: no sum types means literal
unions and tagged live-notification payloads rely on conventions plus linters instead of the
compiler. Because Aureline generates concrete types rather than deriving them in Go, most of Go's
type-system weakness is sidestepped; the residue is exhaustiveness, not expressiveness.

## 1. Language state as of Go 1.26 (current: released 2026-02-10)

What shipped since generics landed in 1.18:

- **1.21–1.22**: type-inference improvements; `database/sql.Null[T]` (generic nullable wrapper,
  [1.22](https://go.dev/doc/go1.22)).
- **1.23**: range-over-func iterators and the [`iter`](https://pkg.go.dev/iter) package
  (`iter.Seq[V]`, `iter.Seq2[K,V]`).
- **1.24**: [full generic type aliases](https://go.dev/doc/go1.24) — `type Rows[T any] = []Row[T]`
  is legal. Useful for codegen-friendly renames across packages.
- **1.25**: [no language changes](https://go.dev/doc/go1.25); `encoding/json/v2` experiment
  (relevant only if Aureline emits JSON codecs; SurrealDB's SDK uses CBOR).
- **1.26**: [two language changes](https://go.dev/blog/go1.26) — self-referential type-parameter
  constraints (`type Adder[A Adder[A]] interface { Add(A) A }`) and `new(expr)` with an initial
  value. `new(expr)` is genuinely useful for optional fields: `Status: new(PostStatus("draft"))`
  replaces the temp-variable dance. Also `errors.AsType[T]()`, a typed error matcher.

What still does not exist, with disposition:

- **Sum types / unions**: the umbrella issue [#19412](https://github.com/golang/go/issues/19412)
  (2017) is still open; [#57644](https://github.com/golang/go/issues/57644) (unions via general
  interfaces) is explicitly "not … adopted in the near future"; newer takes
  [#76920](https://github.com/golang/go/issues/76920) and
  [#80607](https://github.com/golang/go/issues/80607) (Dec 2025-era) are discussion-stage. **Do not
  plan around this landing.**
- **Enums**: [#19814](https://github.com/golang/go/issues/19814) open since 2017, no movement;
  [#36387](https://github.com/golang/go/issues/36387) (exhaustive switch) likewise. `const` +
  defined type remains the only mechanism.
- **Generic methods**: [#49085](https://github.com/golang/go/issues/49085) (900+ upvotes) remains
  open with Ian Lance Taylor's "non-starter unless someone can explain how to implement it"; the
  2025 successor [#77273](https://github.com/golang/go/issues/77273) is exploratory. **Methods
  cannot introduce type parameters; plan APIs around free functions.**

## 2. Prior art: how the best Go DB tools do it

- **[sqlc](https://docs.sqlc.dev/en/latest/tutorials/getting-started-postgresql.html)** — the
  closest model to Aureline. Per query it generates: a `<Name>Params` struct, a `<Name>Row` struct
  (or reuses the table model when the projection matches), and a method on `*Queries` returning
  `(Row, error)` for `:one`, `([]Row, error)` for `:many`, `error` for `:exec`. Nullability is
  decided *at generation time* from schema analysis and encoded as `sql.Null*` types or pointers
  ([`emit_pointers_for_null_types`](https://docs.sqlc.dev/en/latest/reference/config.html)). Its bug
  tracker ([#3710](https://github.com/sqlc-dev/sqlc/issues/3710),
  [#3900](https://github.com/sqlc-dev/sqlc/issues/3900)) shows the failure mode is always
  *inference* (casts, function returns) —
  never the generated shape. Aureline's Rust checker owns inference, so it inherits the shape
  without the failure mode.
- **[ent](https://entgo.io)** — schema-as-code with fully typed builders, but
  projections/aggregations fall back to weakly-typed `Scan`; whole-entity results only are fully
  typed.
- **[sqlboiler](https://github.com/aarondl/sqlboiler)** — database-first, one model per table, query
  mods (`qm.Where("age > ?", 5)`) are stringly typed. Not a model to follow.
- **[jet](https://github.com/go-jet/jet)** — type-safe SQL *builder* (expressions checked at compile
  time) but result mapping is reflection into caller-chosen structs, checked at runtime. Shows the
  ceiling of builder-style APIs without codegen.

Verdict: struct-per-query codegen (sqlc's shape) is both the strongest and the most idiomatic; the
Go community already accepts it.

## 3. The official surrealdb.go SDK today (v1.6.0)

Per [pkg.go.dev](https://pkg.go.dev/github.com/surrealdb/surrealdb.go), typing is
generic-but-shallow, and notably uses **free functions, not methods**, precisely because of the generic-methods
limitation:

```go
func Query[TResult any](ctx, db, sql string, vars map[string]any) (*[]QueryResult[TResult], error)
func Select[TResult any, TWhat TableOrRecord](ctx, db, what TWhat) (*TResult, error)
```

`TResult` is caller-asserted, not checked — a wrong struct surfaces as a CBOR unmarshal error at
runtime. [`models.RecordID`](https://surrealdb.com/docs/reference/golang/api/values/record-id) is
`struct { Table string; ID any }` (CBOR tag 8) — the `ID` is untyped and unbranded. Live
notifications arrive as `chan connection.Notification` with `Action` + `Result any`. Aureline
generating `TResult` per query and wrapping `RecordID` slots directly into this SDK while fixing
exactly its weak points.

## 4. Representing Aureline's type language without unions

- **Literal unions** (`"draft" | "published"`): generate `type PostStatus string` + typed consts.
  Gaps: any string converts (`PostStatus("bogus")` compiles), and switches aren't exhaustive.
  Mitigations: a `Valid()` method checked at bind time (Aureline's runtime already validates at the
  wire boundary), and the [`exhaustive`](https://github.com/nishanths/exhaustive) linter. A closed
  struct (`type PostStatus struct{ v string }` + exported vars) blocks forged values but has an
  awkward zero value and worse ergonomics in composite literals. Recommendation: defined string type
  + consts + `Valid()`; document the gap.
- **Tagged live-notification payloads** (DELETE carries only a record id): three honest options.
  1. *Sealed interface*: `type PostEvent interface{ isPostEvent() }` with `PostCreated{...}`,
     `PostDeleted{ID PostID}` variants. Type switches compile without a default arm and without
     exhaustiveness — a new variant is silently ignorable. Linters (`gochecksumtype`, `exhaustive`)
     recover most of this in CI.
  2. *Generated visitor/matcher*: `func MatchPostEvent[R any](e PostEvent, onCreate
     func(PostCreated) R, onDelete func(PostDeleted) R) R` — this **is** compiler-checked
     exhaustiveness (adding a variant breaks every call site) at the cost of closure noise. Must be
     a free function: methods can't add `R`.
  3. *Struct with optional fields* (`Action` + `*Record` + `*ID`): what SDKs do; illegal states
     representable; weakest. Reject.
  Recommendation: sealed interface as the type, plus a generated `Match` for callers who want the
guarantee. This is the single largest gap vs. Rust enums / TS discriminated unions.
- **Branded record IDs**: `type RecordID[T any] struct { inner models.RecordID }` with `T` a phantom
  parameter (e.g. `RecordID[Product]`). Go permits unused type parameters on types without complaint
  — unlike unused variables — and the pattern is established
  ([phantom types in Go](https://medium.com/@marioraspiantoro/stop-simple-mistakes-in-go-with-phantom-types-3a74504b7f87)).
  Since nothing infers `T`, constructors must be explicit
  (`NewProductID("p1")` — generated per table, which Aureline does anyway) and the wrapper needs
  custom CBOR marshal/unmarshal delegating to `models.RecordID`. This works *well*; `product:1` vs
  `user:1` confusion becomes a compile error, matching the TS/Rust designs.
- **Optional fields**: three candidates. `*T` — idiomatic, `omitempty`-friendly, `new(expr)` (1.26)
  fixes construction; but conflates "absent" and "null", and SurrealDB distinguishes NONE from NULL.
  [`sql.Null[T]`](https://pkg.go.dev/database/sql#Null) — stdlib, explicit `Valid` flag, but no
  combinators and reads database-flavored. Generics `Option[T]` — expressible, but without generic
  methods `opt.Map(f)` can't change type; combinators become free functions and Go programmers won't
  use them. Recommendation: `*T` for `option<T>`; only if a schema actually uses `option<T|null>`
  (tri-state) generate a small three-state wrapper for that field. Don't build an Option library.

## 5. API shape under the generic-methods limitation

A fluent `db.Query(...).Page(10).Each[T](...)` is impossible — methods cannot introduce `T`. But
Aureline generates *per-query* functions, so `T` is always already concrete at the definition site:

```go
// generated: T fixed to ListPostsRow at codegen time
func ListPosts(ctx context.Context, db *aureline.DB, p ListPostsParams) iter.Seq2[ListPostsRow, error]
func LivePosts(ctx context.Context, db *aureline.DB) iter.Seq2[PostEvent, error]
```

`iter.Seq2[Row, error]` (1.23) gives lazy pagination and live streams with plain `for row, err :=
range` loops — no generic method needed anywhere. Shared runtime helpers (`aureline.Collect`,
`aureline.First`) are package-level generic functions, which is exactly how the stdlib (`slices`,
`maps`, `errors.AsType`) and the SurrealDB SDK already work. The limitation costs fluency, not
safety.

## 6. Scalar mappings

| SurrealDB | Generated Go | Notes |
|---|---|---|
| `decimal` | `models.DecimalString` (a `string`) or [shopspring/decimal](https://github.com/shopspring/decimal) | No stdlib decimal; SDK preserves precision as string. Offer a codegen option. |
| `datetime` | `models.CustomDateTime` (wraps `time.Time`) | Nanosecond-capable; CBOR tag on the wire. |
| `duration` | `models.CustomDuration` (wraps `time.Duration`) | Caps at ~292y (int64 ns); SurrealDB durations can exceed — document. |
| `uuid` | `models.UUID` (wraps `uuid.UUID`) | Binary CBOR form. |
| `bytes` | `[]byte` | Direct. |
| `record<t>` | generated `RecordID[T]` wrapper over `models.RecordID` | §4. |
| NONE vs NULL | `models.CustomNil` on the wire; `*T` in generated structs | Tri-state only where schema demands it. |

## 7. Verdict

**Achievable at full fidelity:** per-query params/row structs with exact nullability (sqlc-proven),
branded record IDs via phantom type parameters, typed literal-union consts, `iter.Seq2`
pagination/live streams, generic runtime helpers as free functions. Because the Rust compiler does
all inference, Go never needs mapped types, conditional types, or `keyof` — the features it lacks
relative to TS are mostly features Aureline doesn't need in the target language.

**Falls short of TS/Rust in exactly two places:** (1) no closed sums — literal unions are forgeable
and event switches non-exhaustive without linters or the generated-`Match` pattern; (2) no generic
methods — the API must be function-shaped, not fluent. Neither is a safety hole at the wire boundary
(Aureline validates on decode); both are compile-time-guarantee gaps.

**Closest generated-Go shape:** sqlc's struct-per-query skeleton + phantom-typed IDs +
sealed-interface events with generated `Match` + `iter.Seq2` streaming, layered on `surrealdb.go`'s CBOR
transport. That lands Go materially above every existing SurrealDB Go experience and at parity with
the best Go database tooling — roughly "Rust minus exhaustiveness," which is the honest ceiling of
the language as of Go 1.26.
