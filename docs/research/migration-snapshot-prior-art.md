# Migration snapshots & diffing — prior art (Drizzle v1, Prisma)

> **Provenance** — 2026-08-11. Prior-art for Aureline (statically-checked DSL → SurrealQL DDL).
> Primary sources only: official docs, repos, release notes, design discussions, and the actual
> source of the snapshot serialisers/validators. Everything is linked inline below; the load-bearing
> sources are:
>
> - Drizzle docs: [v0→v1 changes](https://orm.drizzle.team/docs/v0-v1-changes),
>   [upgrade-v1](https://orm.drizzle.team/docs/upgrade-v1),
>   [kit generate](https://orm.drizzle.team/docs/drizzle-kit-generate),
>   [kit up](https://orm.drizzle.team/docs/drizzle-kit-up),
>   [kit check](https://orm.drizzle.team/docs/drizzle-kit-check),
>   [roadmap](https://orm.drizzle.team/roadmap)
> - Drizzle design discussions: [#2832 folder structure v3](https://github.com/drizzle-team/drizzle-orm/discussions/2832),
>   [#2624 updated migration process](https://github.com/drizzle-team/drizzle-orm/discussions/2624),
>   [#5528 snapshot-less generation](https://github.com/drizzle-team/drizzle-orm/issues/5528)
> - Drizzle releases [beta.2](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-beta.2),
>   [rc.2](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-rc.2),
>   [rc.4](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-rc.4);
>   [HINTS.md @ rc.4](https://github.com/drizzle-team/drizzle-orm/blob/v1.0.0-rc.4/drizzle-kit/HINTS.md)
> - Drizzle v1 source, `beta` branch: [`drizzle-kit/src`](https://github.com/drizzle-team/drizzle-orm/tree/beta/drizzle-kit/src)
>   (`dialects/postgres/{snapshot,ddl,diff,versions,serializer}.ts`, `cli/{prompts,hints}.ts`,
>   `cli/commands/generate-common.ts`, `commutativity/types.ts`)
> - Prisma: [shadow database](https://www.prisma.io/docs/orm/prisma-migrate/understanding-prisma-migrate/shadow-database),
>   [migration histories](https://www.prisma.io/docs/orm/prisma-migrate/understanding-prisma-migrate/migration-histories),
>   [troubleshooting](https://www.prisma.io/docs/orm/prisma-migrate/workflows/troubleshooting),
>   [`migrate diff`](https://www.prisma.io/docs/cli/migrate/diff),
>   and the engine's own [`schema-engine/ARCHITECTURE.md`](https://github.com/prisma/prisma-engines/blob/main/schema-engine/ARCHITECTURE.md)
>
> Caveat: Drizzle's v1 source lives on the moving `beta` branch; those links may drift. Read at the
> commit reachable from `beta` on 2026-08-11.

---

## 1. Drizzle — the v0 model, and what broke

### 1.1 v0 layout

```
drizzle/
├─ meta/
│  ├─ _journal.json
│  ├─ 0000_snapshot.json
│  └─ 0001_snapshot.json
├─ 0000_public_electro.sql
└─ 0001_perpetual_sebastian_shaw.sql
```
(verbatim from [#2832](https://github.com/drizzle-team/drizzle-orm/discussions/2832))

A v0 snapshot was a **nested, database-shaped tree** keyed by name — a serialised catalogue:

```json
{ "version": "7", "dialect": "postgresql", "id": "…", "prevId": "…",
  "tables": { "users": { "name": "users", "schema": "",
                         "columns": { "id": { … } }, "indexes": { … } } },
  "enums": {}, "schemas": {}, "views": {}, "_meta": { "tables": {}, "columns": {} } }
```

Note `prevId: string` (singular) and `_meta`, which held resolved rename mappings as
`{ newName: oldName }`. `_journal.json` recorded the ordered migration list (`idx`, `when`, `tag`,
`breakpoints`) and, per [#2832](https://github.com/drizzle-team/drizzle-orm/discussions/2832), kept
the snapshots "as a linked list" specifically "to proactively detect race conditions".

### 1.2 The three failures

**(a) The journal was a git-conflict magnet.** Every migration appended a line to a single shared
file. The official v1 notes give the motive verbatim: the changes "eliminate potential Git conflicts
with the journal file and simplify the process of dropping or fixing conflicted migrations"
([v0→v1](https://orm.drizzle.team/docs/v0-v1-changes)). The roadmap item is blunt: *"Migrate to
folder v3, remove journal."*

**(b) `prevId: string` forced a linear history that git does not have.** From
[#2832](https://github.com/drizzle-team/drizzle-orm/discussions/2832): when generating, Drizzle takes
the newest snapshot in the repo and diffs the TypeScript schema against it — but "the latest json
snapshot might now be a forked version with race condition." Two branches each generate from the
same parent; on merge, the linked list is broken and the newer snapshot silently encodes a base
state that never existed. The maintainer floated three fixes (require a DB connection at generate
time; merge forked snapshots; a hybrid) and concluded **"neither of the ways you can do migrations
is a silver bullet"** — the chosen direction was better *communication* of the conflict rather than
automatic resolution.

**(c) The nested tree was slow and hard to diff.** [beta.2](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-beta.2)
records the rewrite as "Migrated from database snapshots to database **DDL snapshots**" plus
"Reworked the entire architecture for detecting and applying diffs", introspection going ~10s → <1s.

Orthogonally, [#2624](https://github.com/drizzle-team/drizzle-orm/discussions/2624) attacked
apply-time weaknesses: `IF NOT EXISTS` in generated SQL "will just let your migration succeed
without indicating that you have the same table"; no failure status recorded on partial application.

### 1.3 The v1 model

Folder layout (one directory per migration, no central index):

```
drizzle/
├─ 20240823160430_public_electro/
│  ├─ snapshot.json
│  └─ migration.sql
└─ 20240823160431_perpetual_sebastian_shaw/
   ├─ snapshot.json
   └─ migration.sql
```

14-digit UTC timestamp + name; lexicographic ordering; "removing a folder drops unnecessary
migrations" (so `drizzle-kit drop` was deleted). The DB-side ledger gained `name` (full folder name)
and `applied_at`; "migrations are now matched by their full folder name instead of timestamps", and
the migrator "detects and applies **every missing migration**, regardless of timestamp ordering."

The snapshot itself is now a **flat, tagged entity list**. Constructor, verbatim from
[`postgres/snapshot.ts`](https://github.com/drizzle-team/drizzle-orm/blob/beta/drizzle-kit/src/dialects/postgres/snapshot.ts):

```ts
export const toJsonSnapshot = (ddl, prevIds: string[], renames: string[]): PostgresSnapshot => {
  return { dialect: 'postgres', id: randomUUID(), prevIds, version: '8', ddl: ddl.entities.list(), renames };
};
```

Four things changed at once:

1. **`ddl: Entity[]`** replaces the nested `tables`/`enums`/`views` tree. Every entity carries an
   `entityType` discriminator and is identified by the tuple `${schema}:${table}:${name}:${entityType}`
   ([`dialect.ts`](https://github.com/drizzle-team/drizzle-orm/blob/beta/drizzle-kit/src/dialects/dialect.ts)).
   Columns, indexes, PKs, uniques, checks, FKs, policies, privileges are all peers of tables — a
   column is not nested inside its table, it just names it. Diffing becomes one generic set-diff over
   a keyed collection instead of 14 bespoke tree walks.
2. **`prevIds: string[]`** replaces `prevId: string` — the history is explicitly a DAG.
   [rc.2](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-rc.2): "New snapshots now
   collect **all open leaf snapshot IDs** and write them as parents, instead of keeping only one
   latest parent."
3. **`renames: string[]`** is persisted *in the snapshot*, formatted `schema.table.from->schema.table.to`
   ([`prepareMigrationRenames`](https://github.com/drizzle-team/drizzle-orm/blob/beta/drizzle-kit/src/utils/index.ts)).
   The human's disambiguation answer becomes a durable, reviewable artifact rather than a one-shot
   terminal decision.
4. **`version` is per-dialect** — Postgres is at `'8'`, SQLite at `'7'`. Snapshots are validated
   strictly on read (`snapshotValidator.strict(...)`); a wrong version is a hard error, not a warning.

### 1.4 Diff + ambiguity resolution

`ddlDiff(ddlPrev, ddlCur, …)` takes **fourteen resolvers**, one per entity kind
([`diff.ts`](https://github.com/drizzle-team/drizzle-orm/blob/beta/drizzle-kit/src/dialects/postgres/diff.ts)).
Each has the same shape:

```ts
type Resolver<T> = (it: { created: T[]; deleted: T[] })
  => Promise<{ created: T[]; deleted: T[]; renamedOrMoved: { from: T; to: T }[] }>;
```

Pipeline: plain set-diff → hand `{created, deleted}` to the resolver → apply returned renames as
*in-place mutations of the previous DDL* → re-diff. The resolver short-circuits when
`created.length === 0 || deleted.length === 0` — ambiguity exists only when something appeared *and*
something disappeared within the same entity kind. Prompt text: `Is <entity> created or renamed from
another <entity>?`, with a select-list of the deleted candidates.

**The non-interactive contract** was added late, in [rc.4](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-rc.4).
Under `--output json` or a non-TTY stdin, prompts become a request/response protocol
([HINTS.md](https://github.com/drizzle-team/drizzle-orm/blob/v1.0.0-rc.4/drizzle-kit/HINTS.md)):

```jsonc
// emitted
{ "status": "missing_hints",
  "unresolved": [ { "type": "rename_or_create", "kind": "column",
                    "entity": ["public", "users", "display_name"] } ] }
// replied via --hints / --hints-file
[ { "type": "rename", "kind": "column",
    "from": ["public","users","full_name"], "to": ["public","users","display_name"] } ]
```

Three hint types: `rename`; `create` ("this really is new, do not pair it with the deletion"); and
`confirm_data_loss`, whose `reason` catalogue is `non_empty` / `table_recreate` / `type_change`. The
last is **push-only in practice** — `non_empty` means "runtime probed the target entity and found at
least one row." `generate-postgres.ts` wires up only the 14 rename/create resolvers; offline
generation cannot probe, so it never asks for data-loss approval.

### 1.5 Snapshot format migration — the real cost signal

[`drizzle-kit up`](https://orm.drizzle.team/docs/drizzle-kit-up) exists because "it's required
whenever we introduce breaking changes to the json snapshots of the schema and upgrade the internal
version." [`versions.ts`](https://github.com/drizzle-team/drizzle-orm/blob/beta/drizzle-kit/src/dialects/postgres/versions.ts)
is the accumulated bill: a chained ladder `upToV8 → updateUpToV7 → updateUpToV6 → updateToV5`, with
`if (Number(it.version) < 4) throw new Error('Snapshot version <4');` — v1–v3 snapshots are simply
**unrecoverable**. `upToV8` returns `{ snapshot, hints: string[] }`: the upgrade is *lossy* and hands
back a list of what it could not faithfully reconstruct. It also has to synthesise the new `renames`
array by back-deriving it from the old `_meta` side-car
(`Object.entries(json._meta.columns).map(([k, v]) => \`${v}->${k}\`)`, etc.) — i.e. information the
snapshot was never designed to hold had to be reconstructed from a field kept for another purpose.

### 1.6 Commutativity check (v1's answer to the fork problem)

`drizzle-kit check` "lets you check consistency of your generated SQL migrations history" and
"detects non-commutative migrations across branches — e.g. two branches altering the same column, or
one renaming a table that another is altering."
[`commutativity/types.ts`](https://github.com/drizzle-team/drizzle-orm/blob/beta/drizzle-kit/src/commutativity/types.ts)
walks the DAG, finds the common `parentId`, and pairs conflicting statements by `ConflictTarget`
(`{kind, name, schema?, table?}`). `--ignore-conflicts` exists but is discouraged: "if there is a
situation you want to use it, then there is a big chance that `drizzle-kit` didn't check migrations
right and it's a bug." Where branches *are* commutative, `check` feeds the merged parent snapshot +
statements back into `generate` so the next migration diffs against the merged state.

### 1.7 Dissent worth noting

[Issue #5528](https://github.com/drizzle-team/drizzle-orm/issues/5528) asks for snapshot-less
generation (diff schema against the live DB). Arguments: snapshot files are repo noise at scale;
out-of-order PR merges break snapshot lineage; the live DB is guaranteed to be the real state
whereas snapshots can drift. **No maintainer response found** as of this date — treat it as an open
tension, not a decision.

---

## 2. Prisma — shadow database instead of a snapshot

**What it is.** "A second, *temporary* database that is created and deleted automatically each time
you run `prisma migrate dev` and is primarily used to detect problems such as schema drift or
potential data loss."

**How `migrate dev` uses it.** (a) Reset/create the shadow DB → replay the *entire* existing
migration history into it → **introspect** it to recover "the current state" → compare against the
dev database; a mismatch is **drift**. (b) Compute the target schema from the PSL, diff it against
that replayed end-state, render SQL into a new migration folder, evaluate data loss, apply.
`migrate deploy` and `migrate resolve` never touch it: "the only core feature of Migrate that relies
on the shadow database is generating migrations."

**Why replay rather than record?** Stated explicitly in the engine's architecture doc:

> "The shadow database is the only mechanism by which Migrate can determine what migrations do. From
> Migrate's perspective, Migrations are **black boxes**: we do not parse SQL… The only way to figure
> out what the effect of a migration is is to run it."

Migrations "can contain arbitrary SQL, including database features that cannot be represented in the
Prisma schema… like check constraints and views. Since these can't be diffed nor rolled back, the
only way migrate has to make sure that the database schema state actually matches the migrations… is
to reset the database and reapply them." That is the whole argument: **Prisma chose an escape hatch
(hand-edited arbitrary SQL) and paid for it with a live database at generate time.**

**Not confirmed:** no primary source in Prisma's docs, engine repo, or blog considers and rejects a
*committed snapshot file* as an alternative. They argue against fully-declarative migrations, which
is a different question. Do not attribute an anti-snapshot rationale to Prisma.

**History storage.** `prisma/migrations/<timestamp>_<name>/migration.sql`, plus a top-level
`migration_lock.toml` holding only `provider = "postgresql"` (detects provider switches → `P3019`).
The docs are emphatic: "The `migrations` folder is the **source of truth** for the history of your
data model"; source-controlling `schema.prisma` alone is not enough, and `migrate deploy` "*only*
runs migration files. It does not use the Prisma schema."

**Apply-time ledger** — `_prisma_migrations` (DDL confirmed in
[`flavour/postgres.rs`](https://github.com/prisma/prisma-engines/blob/main/schema-engine/connectors/sql-schema-connector/src/flavour/postgres.rs)):
`id`, `checksum` (sha256 of the migration file, never overwritten), `finished_at`, `migration_name`,
`logs`, `rolled_back_at`, `started_at`, `applied_steps_count` (deprecated). `started_at` set with
neither `finished_at` nor `rolled_back_at` = failed migration.

**Two distinct detectors, and the docs say so.** Checksum mismatch → *migration history conflict*
(file edited or deleted). Shadow-DB replay comparison → *schema drift* (database changed out of
band). "The shadow database is not responsible for checking if a migration file has been edited or
deleted."

**Costs.** Requires `CREATEDB`/superuser (PostgreSQL) or `CREATE, ALTER, DROP, REFERENCES ON *.*`
(MySQL); failure is `P3014`. "Some cloud providers do not allow you to drop and create databases
with SQL… and some really limit you to 1 database" (Heroku, Digital Ocean, Vercel Postgres named) —
those need a manual `shadowDatabaseUrl`, guarded by `P3025` because pointing it at the real DB
"might delete all the data in your database." Azure SQL hard-blocks auto-creation (`P3020`). CI is
sidestepped rather than solved: `deploy` never uses a shadow DB and never detects drift, because
"many people would not be comfortable with creating/using temporary databases being on the
deployment path." `migrate diff --from-schema`/`--to-schema` is shadow-free, but
`--from-migrations`/`--to-migrations` still needs one — reading a migrations directory's end-state
means replaying opaque SQL.

---

## 3. Direct comparison

| | Drizzle (committed snapshot) | Prisma (shadow DB replay) |
|---|---|---|
| Prior state comes from | `snapshot.json` in the repo | replaying migration SQL into a temp DB, then introspecting |
| Generate-time deps | none — pure file I/O | a live server + `CREATE DATABASE` rights |
| Handwritten/arbitrary DDL | breaks the model (snapshot no longer describes reality) | fully supported — that's the point |
| Renames | asked interactively, answer persisted in the snapshot | inferred from the SQL diff; `@map` used to decouple names |
| Drift vs. reality | **undetectable at generate time** | detected on every `migrate dev` |
| Branching | DAG `prevIds` + `check` commutativity report | reset-the-database; conflicts surface as drift |
| Format evolution | a real, recurring tax (`kit up`, 8 versions, lossy, v<4 unrecoverable) | none — nothing durable to version except the SQL |
| Review surface | snapshot diff is readable in PRs | nothing to review beyond the SQL |

**Failure mode, Drizzle:** the snapshot lies. Someone edits the DB by hand or writes a `--custom`
migration; the snapshot silently diverges, and the next `generate` computes a migration from a state
that no longer exists. Plus the format-version tax, forever.

**Failure mode, Prisma:** `migrate dev` demands a database reset — switching branches, editing a
migration, and fiddling with the DB are the three causes the docs name. And you cannot generate a
migration at all without a live, privileged database.

---

## 4. What Aureline should take

**From Drizzle (adopt):**
- **Flat list of tagged entities**, not a nested tree. `{ entityType, table?, name, …props }` with a
  stable identity tuple. This was the single headline change in v1 and it makes the diff one generic
  keyed set-diff. Aureline starts with `tables` and `fields`; a nested `{ tables: { fields: {} } }`
  shape will have to be flattened later, and flattening is a format-version migration.
- **`version` on the artifact from commit #1**, with a strict validator that rejects unknown
  versions loudly, and a documented upgrade function per bump that may return `hints[]` for anything
  it cannot recover.
- **`prevIds: string[]`, never `prevId: string`.** Model history as a DAG on day one. Drizzle spent a
  major version fixing this.
- **No central journal.** One folder per migration containing its snapshot + its `.surql`. Deleting a
  folder deletes a migration. Drizzle removed `_journal.json` explicitly for git-conflict reasons.
- **Persist resolved ambiguity in the snapshot** (Drizzle's `renames: string[]`). Even with no
  SurrealDB rename DDL, "the user said `email` became `email_address`" is a durable fact: it stops
  Aureline re-asking, it is reviewable in the PR diff, and it is what a data-backfill hint keys off.
- **Build the interactive prompt and its non-interactive twin together.** Drizzle bolted `--hints` /
  `missing_hints` on at rc.4. The resolver should be the *only* interactive point in the pipeline,
  and should have a JSON request/response contract from the start.

**From Prisma (adopt the ideas, not the mechanism):**
- An apply-time ledger table in SurrealDB modelled on `_prisma_migrations`: migration name, content
  **checksum**, `started_at` / `finished_at` / `rolled_back_at`. Complementary to, not a substitute
  for, the generate-time snapshot — checksums catch *edited migration files*, which a snapshot can't.
- A `migration_lock`-style pin (SurrealDB version / Aureline DSL version) so an incompatible
  toolchain fails fast rather than mis-generating.
- The negative lesson: Prisma needs a shadow DB *because* migrations are opaque SQL it refuses to
  parse. Aureline's snapshot is trustworthy only while Aureline is the sole author of the DDL. Treat
  "hand-edit the generated `.surql`" as the feature that would force a shadow DB later; gate it, mark
  such migrations explicitly, and mark the resulting snapshot advisory.

**Aureline-specific consequence of "SurrealDB has no rename":**
The absence of rename DDL does **not** reduce the value of the snapshot — it raises it. Because a
rename must be `REMOVE FIELD` + `DEFINE FIELD`, the *only* thing standing between the user and
silent data loss is a warning, and a warning requires knowing what existed before. Also note the
Drizzle split: `rename_or_create` is resolvable offline, but `confirm_data_loss(non_empty)` requires
probing a live table. Aureline has no probe at generate time, so it must warn on **every** removal
unconditionally, and should surface the removal set as a first-class, machine-readable part of the
generate output — not as log noise.

---

## 5. Recommendation on decision (2): snapshot on day one

**Adopt a snapshot now.** Not a rich one — a minimal, versioned, flat-entity one — but now.

The reasoning, plainly:

1. **"Plain idempotent desired-state DDL" is not a simpler v0 of the same product; it is a different,
   strictly weaker product.** `DEFINE … OVERWRITE` covers add and modify. It can never cover
   *removal*, because removal is defined by absence, and absence is only visible relative to a prior
   state. A desired-state-only Aureline can never say "this drops `user.email` and its data." Given
   the SurrealDB rename constraint, that warning is the single most valuable thing the tool can
   produce. Shipping without it teaches users the tool is safe when it isn't.

2. **The retrofit path is worse than Drizzle's, not comparable to it.** Drizzle's `kit up` ladder is
   painful — 8 versions, lossy, `< 4` unrecoverable — but at every step it had *a prior snapshot to
   upgrade from*. Aureline retrofitting from nothing has only two ways to reconstruct prior state:
   introspect a live database (explicitly ruled out at generate time), or parse back its own emitted
   SurrealQL — which is precisely the opaque-migration problem that forced Prisma into a shadow
   database. Deferring the snapshot doesn't defer the cost; it converts a format-migration problem
   into an architecture problem.

3. **The cost right now is near zero, and it only ever goes up.** With tables and fields only, the
   snapshot is a few hundred bytes of flat entities and the diff is a set difference. The expensive
   part of Drizzle's history was never *having* snapshots — it was having shipped a nested,
   linear-history, rename-in-a-side-car format with no versioning discipline, then paying for it
   seven times. Aureline can copy the v1 destination directly and skip the journey.

4. **Being a bespoke DSL is an advantage here, and it argues for the snapshot.** Aureline's snapshot
   records its own semantic model, not an introspected catalogue. There is no impedance mismatch to
   drift, no vendor catalogue to track, and the snapshot doubles as the checker's canonical IR. That
   is a cheaper, more stable artifact than anything Drizzle or Prisma has.

**Concretely, the day-one artifact:**

```jsonc
{
  "version": 1,
  "id": "<uuid>",
  "prevIds": [],                       // array from the start, even though it's length 0 or 1 today
  "renames": [],                       // durable record of resolved rename decisions
  "ddl": [
    { "entityType": "table", "name": "user", "schemafull": true },
    { "entityType": "field", "table": "user", "name": "email", "type": "string", "optional": false }
  ]
}
```

One folder per migration: `<utc-timestamp>_<name>/{snapshot.json, migration.surql}`. No journal. A
strict validator that rejects `version != 1`. An `aureline up` command that does nothing yet — but
exists, and has a test, so that the second version has somewhere to land.

**What to defer:** commutativity checking (`drizzle-kit check`), down-migrations, DAG merging of
multiple leaves, and the apply-time ledger table. Those are all additive on top of `prevIds: []` and
per-migration folders. The DAG *shape* must be there on day one; the DAG *reasoning* need not be.
