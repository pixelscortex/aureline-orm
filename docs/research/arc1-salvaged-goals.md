# Arc 1 — Salvaged Product Goals

**Provenance.** Salvaged 2026-08-11 from the issue tracker of attempt one, `pixelscortex/aureline`
(the project was called "Aurora" for most of that tracker's life). 95 issues read in full — 93 open,
2 closed, numbered #1–#123 with gaps where PRs sit. Bodies and comments came from GitHub, not from
the archived code.

**What this file is.** Durable product and language intent only. Issues about the old crate layout,
old module names, CI/release plumbing, and specific bug fixes in the dead implementation have been
dropped. Where an issue mixed a goal with an implementation task, only the goal survived.

**Why arc 1 accumulated ~93 open issues.** The tracker was used as a design notebook, not a work
queue. Most issues are multi-page specs with syntax sketches, SurrealDB doc citations, cross-language
API tables, and explicit "blocked on / required for" chains — they were written to *think*, and
almost none were ever closable in one sitting. Two structural things compounded it. First, a single
critical-path item (the SurrealQL escape hatch, #14) gated events, functions, access, permissions,
asserts and views, so roughly a dozen issues sat permanently blocked behind one unshipped feature.
Second, exhaustive enumeration: one tracker (#67) checklists all ~411 SurrealDB stdlib functions
across 27 namespaces, and the ALTER audit (#55) spawned eight near-identical sub-issues. The scope
was mapped far ahead of what the implementation could absorb, and the map itself became the backlog.

---

## Language surface

**Full DEFINE-statement coverage as first-class DSL blocks.**
The schema language should eventually express everything SurrealDB can define — not just tables,
fields and indexes, but events, access methods, system users, global params, sequences,
user-defined functions, buckets, ML models and analyzers — each with native, non-SurrealQL-shaped
syntax where a clean DSL form exists.
Source: [#32](https://github.com/pixelscortex/aureline/issues/32), [#33](https://github.com/pixelscortex/aureline/issues/33), [#34](https://github.com/pixelscortex/aureline/issues/34), [#35](https://github.com/pixelscortex/aureline/issues/35), [#36](https://github.com/pixelscortex/aureline/issues/36), [#37](https://github.com/pixelscortex/aureline/issues/37), [#38](https://github.com/pixelscortex/aureline/issues/38), [#39](https://github.com/pixelscortex/aureline/issues/39), [#50](https://github.com/pixelscortex/aureline/issues/50), [#52](https://github.com/pixelscortex/aureline/issues/52), [#53](https://github.com/pixelscortex/aureline/issues/53), [#54](https://github.com/pixelscortex/aureline/issues/54)

**Graph edges as first-class `relate` blocks.**
Edge tables get their own keyword with `in -> out` declarations, union targets on either side,
`@enforced` referential integrity, ordinary data fields, and the `[in, out]` unique-index pattern.
Directional only — SurrealQL has no symmetric RELATE, and traversal works both ways anyway.
Source: [#48](https://github.com/pixelscortex/aureline/issues/48)

**Views as incrementally-maintained materialized tables.**
A `view` block compiles to `DEFINE TABLE ... AS SELECT`. These are not SQL views: SurrealDB refreshes
them incrementally when the FROM table changes. They must surface as read-only typed descriptors so
writes to a view fail at compile time in the host language.
Source: [#49](https://github.com/pixelscortex/aureline/issues/49), [#14](https://github.com/pixelscortex/aureline/issues/14)

**Type unions and literal types.**
`record<user | bot>` for heterogeneous references, `int | string` for primitive unions, and literal
types (`"draft" | "published"`, `200 | 404`, object shapes). Without literals, every status-like field
degrades to free-form `string` and the generated client stops catching invalid values.
Source: [#65](https://github.com/pixelscortex/aureline/issues/65), [#66](https://github.com/pixelscortex/aureline/issues/66)

**Field clauses and validation attributes.**
`@default`, `@value`, `@readonly`, `@reference(on_delete:)`, `@comment`, plus `@assert` in several
authoring forms. Above them sits curated sugar (`@email()`, `@lowercase()`, `@url()`, `@uuid()`)
that lowers to the corresponding `ASSERT` / `VALUE` clause, with the raw form always available
underneath.
Source: [#42](https://github.com/pixelscortex/aureline/issues/42), [#44](https://github.com/pixelscortex/aureline/issues/44), [#46](https://github.com/pixelscortex/aureline/issues/46)

**Reusable, type-checked assert helpers.**
Top-level `assert is_even(input: number) { ... }` definitions usable across fields, with the helper's
input type checked against every use site, and edits to a helper propagating as diff entries for
every field that references it.
Source: [#45](https://github.com/pixelscortex/aureline/issues/45)

**Permissions as one uniform `@allow` concept, not per-entity blocks.**
Late in arc 1 the design converged from bespoke `@permissions { ... }` blocks onto a single generic
attribute — `@allow(select, #surql { WHERE ... })` on fields, `@@allow(...)` on tables, and the same
shape for functions (`@allow(run, ...)`). Semantic validation decides which operations are legal in
each context. This keeps the attribute parser generic instead of growing SurrealDB-shaped grammar.
Source: [#43](https://github.com/pixelscortex/aureline/issues/43), [#50](https://github.com/pixelscortex/aureline/issues/50), [#37](https://github.com/pixelscortex/aureline/issues/37)

**Flexible object items inside collection types.**
A schemafull `array<object>` field silently rejects nested keys unless the generated `.*` item field
is FLEXIBLE. The language needs a way to express item-level flexibility — the attribute cannot just
apply to the outer array type. Reported from real use, not theory.
Source: [#121](https://github.com/pixelscortex/aureline/issues/121)

**Automatic authenticated-identity population.**
Declaring that an `author` / `owner` field is auth-owned should make creation fill it from the
authenticated identity, instead of every call site writing `$auth.id` by hand. Open questions around
admin/seed/test overrides and what happens with no session.
Source: [#110](https://github.com/pixelscortex/aureline/issues/110)

**Multi-scope schemas via location annotations.**
Let one schema describe data spread across several namespaces/databases, Postgres-schema style, with
an `@location(ns:, db:)` annotation on tables. Unresolved: whether the generated client gets one root
or one per scope, and what a cross-scope move means for migration.
Source: [#41](https://github.com/pixelscortex/aureline/issues/41)

---

## The SurrealQL escape hatch

**A raw-SurrealQL escape hatch is the load-bearing feature of the whole language.**
Event bodies, function bodies, signup/signin expressions, permission WHERE clauses, inline asserts
and view SELECTs are all SurrealQL. A `#surql { ... }` block that passes through verbatim unblocks
all of them at once; arc 1 repeatedly identified this as critical path and repeatedly failed to ship
it first.
Source: [#14](https://github.com/pixelscortex/aureline/issues/14), [#3](https://github.com/pixelscortex/aureline/issues/3), [#88](https://github.com/pixelscortex/aureline/issues/88)

**An inline shorthand for one-liners.**
`#s\`string::lowercase($value)\`` parses to the same AST node as a `#surql { }` block, for the very
common single-expression `@value` / `@assert` / `@allow` case. No whitespace between `#s` and the
backtick; backticks chosen so SurrealQL string quotes stay usable inside. This one actually shipped.
A narrower, never-parsed `#raw\`...\`` variant was proposed and left undecided — the worry was it
becomes a haven for unmaintained syntax that no tooling can migrate.
Source: [#92](https://github.com/pixelscortex/aureline/issues/92), [#47](https://github.com/pixelscortex/aureline/issues/47)

**Then: a typed superset over the embedded SurrealQL.**
The second stage is the ambitious one. Don't extend SurrealDB — add a static semantic layer in front
of it: parse the embedded SurrealQL, lower to a small owned Query IR, resolve table/field/param
references against the schema, infer the output row shape, and hand that to codegen. Typing is
context-sensitive (a view expects a selectable row shape, an assert expects a boolean, permissions
expect a WHERE-style expression), so it can grow one context at a time rather than covering all of
SurrealQL on day one. Where inference fails, demand an explicit annotation rather than guessing.
Source: [#14](https://github.com/pixelscortex/aureline/issues/14), [#91](https://github.com/pixelscortex/aureline/issues/91), [#49](https://github.com/pixelscortex/aureline/issues/49)

**A SurrealDB function signature catalog as shared infrastructure.**
One data file of every stdlib function — namespace, params, return type, variadic/closure flags,
schema-tied markers — feeding the type checker, LSP completion and hover, assert-helper checking, and
potentially the generated runtime bindings. Nothing type-checks inside `#surql` without it.
Source: [#13](https://github.com/pixelscortex/aureline/issues/13)

---

## Schema, migration, and the database boundary

**Migrations are derived by diffing declared schema against a recorded snapshot.**
The engine keeps an explicit, auditable list of which entity kinds it can diff, and a feature is only
"migration-ready" when its diff/render/apply behaviour exists — parser support for a block never
implies migration support for it. Arc 1 wrote that rule down after repeatedly conflating the two.
Squashing a long history into one migration plus a fresh baseline snapshot was recognised as
eventually necessary and deferred for want of any deployment with real history.
Source: [#3](https://github.com/pixelscortex/aureline/issues/3), [#88](https://github.com/pixelscortex/aureline/issues/88), [#7](https://github.com/pixelscortex/aureline/issues/7)

**Prefer in-place ALTER over REMOVE+DEFINE wherever SurrealDB allows it.**
The single biggest data-safety win. A per-entity audit established exactly what is alterable in place
versus what forces a destructive redefine — this table is worth re-deriving rather than re-guessing.
Source: [#55](https://github.com/pixelscortex/aureline/issues/55), [#56](https://github.com/pixelscortex/aureline/issues/56), [#57](https://github.com/pixelscortex/aureline/issues/57), [#58](https://github.com/pixelscortex/aureline/issues/58), [#59](https://github.com/pixelscortex/aureline/issues/59), [#60](https://github.com/pixelscortex/aureline/issues/60), [#61](https://github.com/pixelscortex/aureline/issues/61)

**Classify destructiveness and confirm before applying it.**
Distinguish *data loss* (a table physically dropped) from *data invalidation* (rows survive but no
longer satisfy the new schema) — the two need very different warning text. Enumerate destructive ops
before apply, print exactly what will be lost, prompt, and offer a `--yes` bypass for CI.
Source: [#9](https://github.com/pixelscortex/aureline/issues/9), [#62](https://github.com/pixelscortex/aureline/issues/62), [#63](https://github.com/pixelscortex/aureline/issues/63)

**Emit operations in dependency-safe order, deliberately.**
Ordering of emitted operations must be an explicit pass with a phase model (drop dependents → drop
fields → drop containers → create containers → add fields → add dependents), not an accident of the
order the differs happen to run in. Related: an analyzer change must schedule `REBUILD INDEX` for
every full-text/HNSW index that references it.
Source: [#8](https://github.com/pixelscortex/aureline/issues/8), [#64](https://github.com/pixelscortex/aureline/issues/64)

**Applying a migration and recording it must not be able to disagree.**
If the DDL succeeds and the tracking write fails, the next run re-applies a migration that already
landed. Wrap both in one transaction if SurrealDB permits, otherwise use a pending→applied state
machine, plus a `verify` command (journal vs tracking rows vs checksums) and an explicit
`mark-applied` escape for humans. Never record "applied" before the SQL runs.
Source: [#109](https://github.com/pixelscortex/aureline/issues/109)

**Introspect an existing database, and detect drift against a live one.**
Read live schema metadata, parse it back into the schema model, and emit idiomatic source text — the
adoption path for existing SurrealDB projects. The same emitter powers the formatter, and the same
introspection powers read-only drift detection against a deployed database.
Source: [#5](https://github.com/pixelscortex/aureline/issues/5), [#6](https://github.com/pixelscortex/aureline/issues/6), [#12](https://github.com/pixelscortex/aureline/issues/12)

**Index builds must be safe on live, large tables.**
Support concurrent/deferred index construction so a migration doesn't block production writes, and
poll for build completion rather than assuming it finished.
Source: [#51](https://github.com/pixelscortex/aureline/issues/51)

**Round-trip correctness proven against a real SurrealDB, cheaply.**
The Rust SDK's embedded in-memory mode spins a real instance inside the test process in
milliseconds — no container, no network, no shared state. Target shape: per-entity
declare → migrate → apply → introspect → assert, plus up/down round-trips, drift detection, and
ALTER-preserves-rows tests, whole suite under a minute.
Source: [#31](https://github.com/pixelscortex/aureline/issues/31)

---

## Static checking and diagnostics

**One semantic model shared by the checker, the LSP, and codegen.**
Every consumer should read a resolved, flattened view of the schema — symbol tables, stable IDs,
source-backed ranges, cross-reference maps — rather than each re-walking the AST and re-implementing
name resolution. Two constraints learned the hard way: the model must tolerate unresolved references
so an editor can report many diagnostics at once, and a map-based index silently swallows duplicate
declarations, so duplicate detection needs deliberate handling.
Source: [#82](https://github.com/pixelscortex/aureline/issues/82), [#113](https://github.com/pixelscortex/aureline/issues/113), [#122](https://github.com/pixelscortex/aureline/issues/122), [#123](https://github.com/pixelscortex/aureline/issues/123)

**Whole-schema validation, not per-table validation.**
Full-text indexes must reference declared analyzers; `record<x>` must reference a declared table;
top-level names and per-table index names must be unique. Downstream layers trust the checker rather
than re-validating.
Source: [#2](https://github.com/pixelscortex/aureline/issues/2)

**Tolerant parsing with recovery.**
Keep parsing past a syntax error so the rest of the file still yields diagnostics and completions —
the LSP spends its whole life looking at invalid documents.
Source: [#84](https://github.com/pixelscortex/aureline/issues/84)

**Compiler-grade diagnostic rendering with did-you-mean suggestions.**
Rust/Prisma-style CLI output: file, line/column, source line, underline, diagnostic code, help text —
rendered from the *same* structured diagnostics the LSP consumes, never a parallel error type.
Suggestions should extend past keywords to type names and semantic names (`duratio` → `duration`,
`recrod<User>` → `record<User>`, near-miss analyzer names).
Source: [#11](https://github.com/pixelscortex/aureline/issues/11), [#90](https://github.com/pixelscortex/aureline/issues/90), [#82](https://github.com/pixelscortex/aureline/issues/82)

**Normalize errors that come from SurrealDB's own parser.**
When validation delegates to the SurrealDB parser, its rendered snippets leak into editor messages —
often pointing at a synthetic wrapper query rather than the user's source. Strip the foreign snippet,
keep the message, attach our own range, and add contextual help.
Source: [#91](https://github.com/pixelscortex/aureline/issues/91)

**Check declared signatures against embedded SurrealQL bodies.**
A function's declared parameters should be verified to actually appear in its `#surql` body, by
comparing what the SurrealQL parser found against what was declared — a mismatch is an error.
Source: [#37](https://github.com/pixelscortex/aureline/issues/37)

---

## Code generation and SDKs

**A stable schema IR with a generator contract, and no blessed target.**
Codegen consumes a documented intermediate representation through a single generator interface, so
first-party TypeScript / Rust / Python generators and third-party ones are the same kind of thing.
The long-term ambition was WASM-plugin generators in any language; a concrete near-term test of the
premise was generating for an existing third-party SurrealDB TypeScript ORM as an alternate target.
The point is the no-lock-in story: the schema is the source of truth, the generated client is
replaceable.
Source: [#16](https://github.com/pixelscortex/aureline/issues/16), [#17](https://github.com/pixelscortex/aureline/issues/17), [#21](https://github.com/pixelscortex/aureline/issues/21), [#89](https://github.com/pixelscortex/aureline/issues/89)

**TypeScript first, then Rust, then Python.**
Each generator emits table descriptors, typed row shapes, typed params, typed access signin/signup,
and sequence accessors, all importable as plain values. TS ships first because it validates the
pattern fastest; Rust follows the codegen-then-consume convention Diesel and SeaORM already use;
Python targets pyright with `py.typed` and a strict config emitted at project init.
Source: [#18](https://github.com/pixelscortex/aureline/issues/18), [#19](https://github.com/pixelscortex/aureline/issues/19), [#20](https://github.com/pixelscortex/aureline/issues/20)

**Capability-typed column references.**
A column's generated type encodes what its schema declaration makes possible: only a full-text column
exposes `.matches()`, only an HNSW vector column exposes `.nearest()` (with its dimension in the
type), a record column exposes resolution. The user is never offered a method that cannot work. The
same hierarchy across all three languages, implemented with each one's native tricks.
Source: [#22](https://github.com/pixelscortex/aureline/issues/22)

**Schema-derived names constrain the standard library too.**
Where a SurrealDB function takes a schema-defined name — an analyzer for `search::analyze`, a sequence
for `sequence::next` — codegen emits a literal union of the declared names so typos fail at edit time.
This is the only place the stdlib surface touches codegen at all.
Source: [#67](https://github.com/pixelscortex/aureline/issues/67)

**Deliberate cross-language API symmetry.**
The same operation should read the same way in all three languages, with differences only where the
host language forces them (`::` vs `.`, `==` vs `.eq()`, `from_` vs `from`, await syntax). Arc 1
maintained a side-by-side table as the spec for this and planned the docs site around it.
Source: [#25](https://github.com/pixelscortex/aureline/issues/25), [#30](https://github.com/pixelscortex/aureline/issues/30)

---

## Runtime and client ergonomics

**Standard-library bindings live in the runtime, not in generated code.**
SurrealDB's ~411 stdlib functions across 27 namespaces don't depend on the user's schema, so they are
hand-written once per language and shipped in the runtime package. The runtime only emits SurrealQL
fragments; the database does the work. Getting this boundary wrong would put a four-hundred-function
surface into every generated project.
Source: [#67](https://github.com/pixelscortex/aureline/issues/67)

**Closure-taking functions need a story for server-side predicates.**
`array::map` / `filter` / `fold` take SurrealQL closures that run in the database, so a host-language
lambda cannot be passed through. Two routes: a typed mirror DSL (best ergonomics, large surface,
fragile as SurrealDB evolves) or template-string passthrough (always works, weaker typing inside the
body). Arc 1's call: ship passthrough, add typed builders incrementally for the most-used functions.
Source: [#68](https://github.com/pixelscortex/aureline/issues/68)

**Live queries with a reactive-framework-grade developer experience.**
Explicitly benchmarked against Convex: a typed subscription primitive over `LIVE SELECT`, reference-
counted deduplication so N components sharing a query open one socket subscription, reconnect with
resync, cursor pagination that stays stable as rows arrive at the head, and framework adapters —
React first (`useLiveQuery` / `useMutation`), then Vue/Svelte/Solid, plus a Rust `Stream` and a Python
async generator. Optimistic mutations were flagged as a genuinely hard, separate design problem.
Source: [#69](https://github.com/pixelscortex/aureline/issues/69), [#70](https://github.com/pixelscortex/aureline/issues/70), [#71](https://github.com/pixelscortex/aureline/issues/71), [#72](https://github.com/pixelscortex/aureline/issues/72), [#73](https://github.com/pixelscortex/aureline/issues/73), [#74](https://github.com/pixelscortex/aureline/issues/74), [#75](https://github.com/pixelscortex/aureline/issues/75), [#76](https://github.com/pixelscortex/aureline/issues/76), [#77](https://github.com/pixelscortex/aureline/issues/77)

**Typed graph traversal in the client.**
Declared edges should produce typed traversal in the generated API, not string-built paths — the
payoff for making relations first-class in the language.
Source: [#48](https://github.com/pixelscortex/aureline/issues/48)

**Auth as a generated, typed surface.**
Access methods declared in the schema become typed `signup` / `signin` calls, with parameter names
extracted from the declared expressions. Aimed at the "SurrealDB as the whole backend, no separate
auth server" use case.
Source: [#33](https://github.com/pixelscortex/aureline/issues/33)

---

## Editor and developer tooling

**A real language server, not syntax highlighting.**
Document sync, diagnostics carrying precise source ranges rather than flat strings, and semantic
completion driven by the shared schema model — declarations, primitive and generic types, known table
names inside `record<...>`, known analyzer names, and attributes offered only in valid positions.
Source: [#26](https://github.com/pixelscortex/aureline/issues/26), [#27](https://github.com/pixelscortex/aureline/issues/27), [#86](https://github.com/pixelscortex/aureline/issues/86)

**Embedded SurrealQL is a nested language, not a string.**
The tree-sitter grammar needs a dedicated node for escape-hatch blocks with a SurrealQL grammar
injected into the body, so embedded queries highlight and structure properly. Tree-sitter provides
structure and colour only — type truth stays with the semantic layer.
Source: [#14](https://github.com/pixelscortex/aureline/issues/14), [#92](https://github.com/pixelscortex/aureline/issues/92)

**Editor support across VS Code, Vim/Neovim, and Zed.**
Ship extensions wrapping grammar plus language server. Arc 1's hard-won lesson is about the
contributor loop, not the extensions: grammar development where the parser, highlight queries,
compiled WASM, editor cache, and installed extension path can each drift independently is
unworkable. One documented command must set up local development against uncommitted grammar
changes and clear stale caches.
Source: [#28](https://github.com/pixelscortex/aureline/issues/28), [#29](https://github.com/pixelscortex/aureline/issues/29), [#79](https://github.com/pixelscortex/aureline/issues/79)

**A small, predictable CLI.**
`init` to scaffold a project with sensible defaults, `check` to validate and exit non-zero for CI and
pre-commit hooks, `fmt` for idempotent canonical formatting. Together they are the first-run
experience.
Source: [#10](https://github.com/pixelscortex/aureline/issues/10), [#11](https://github.com/pixelscortex/aureline/issues/11), [#12](https://github.com/pixelscortex/aureline/issues/12)

**Documentation structured as one concept, three languages.**
Every concept page shows the same workflow in TypeScript, Rust and Python side by side — the
cross-language API symmetry goal turned into the reading experience.
Source: [#30](https://github.com/pixelscortex/aureline/issues/30)

**A standalone SurrealQL language server as a possible spin-off.**
Once there is a SurrealQL parser, a function catalog, and expression type inference, that combination
is a general-purpose SurrealDB LSP that the wider ecosystem lacks — usable with no schema at all.
Speculative and explicitly gated on quality and demand, but recorded as a strategic option.
Source: [#15](https://github.com/pixelscortex/aureline/issues/15)

---

## Hard-won constraints worth not rediscovering

**SurrealDB has no rename, anywhere.**
`ALTER` changes metadata only — never an entity's name. The honest default is to treat a removed name
plus a new name as drop-and-create, and let users who need data preserved write the four-step
define/copy/drop/repoint sequence themselves. Rename *detection* was judged genuinely hard and
deferred. Related: index renames currently slip through diffing entirely because only structural
presence is compared.
Source: [#4](https://github.com/pixelscortex/aureline/issues/4), [#108](https://github.com/pixelscortex/aureline/issues/108)

**Two redefines are silently catastrophic.**
Access methods: signup/signin bodies, type and JWT key are immutable, so changing them forces
REMOVE+DEFINE — which invalidates every live token and logs out every signed-in user. Sequences:
only TIMEOUT is alterable, so any START/STEP/BATCH change resets the counter. Both must be flagged
destructive with warning text that names the actual consequence.
Source: [#62](https://github.com/pixelscortex/aureline/issues/62), [#63](https://github.com/pixelscortex/aureline/issues/63)

**SurrealDB's own parser is internal API.**
Its Rust crate documents itself as unstable and makes no semver promise. If it is used at all, pin it
to a supported database version and confine every direct reference to a single adapter module that
converts into an owned IR — so a database upgrade breaks one file, not the compiler.
Source: [#14](https://github.com/pixelscortex/aureline/issues/14)

**`LIVE SELECT` cannot paginate.**
Any pagination over live results is client-side work: hold the cursor locally and merge live events
into a paginated baseline. This is real design effort, not a wrapper.
Source: [#72](https://github.com/pixelscortex/aureline/issues/72)

**Views only react to their FROM table.**
Changes to joined tables don't trigger a refresh, and initial materialization over large data can be
slow. Both are user-visible semantics, not implementation details.
Source: [#49](https://github.com/pixelscortex/aureline/issues/49)

**Analyzer changes leave dependent indexes stale.**
Changing tokenizers or filters does not re-tokenize existing full-text/HNSW indexes; a `REBUILD INDEX`
must be scheduled after the analyzer change and before anything reads from those indexes.
Source: [#64](https://github.com/pixelscortex/aureline/issues/64)

**SurrealQL has more comment syntaxes than you expect.**
Scanners over embedded SurrealQL must handle `--` and `#` line comments alongside `//` and `/* */`, or
they will collect parameters out of commented-out code.
Source: [#112](https://github.com/pixelscortex/aureline/issues/112)
