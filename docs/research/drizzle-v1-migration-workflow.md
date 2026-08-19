# Drizzle v1 migration workflow

> **Provenance — 2026-08-18.** This is a point-in-time account of Drizzle ORM / Drizzle Kit v1's
> new migration design, based only on first-party Drizzle documentation, releases, discussions,
> npm metadata, and source. It is prior art, not an Aureline specification. The implementation
> references are pinned to [`v1.0.0-rc.4`](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-rc.4)
> (`748058e`), with the migration-relevant files checked again at the `rc5` canary source commit
> [`ab785fc`](https://github.com/drizzle-team/drizzle-orm/commit/ab785fcd99710d6d136ffbfd121b7aeb96e4d51d).

## Executive answer

Drizzle v1 did **not** abandon committed snapshots. It abandoned the one shared journal and the
assumption that migration history is a single linked list.

The new model is:

```text
TypeScript schema ------------------------------------> target DDL
                                                          |
migration folders -> snapshot DAG -> checked merge base --+-> DDL diff
                                                               |
                              rename/create decisions ---------+
                                                               v
                                             JSON DDL statements
                                               /              \
                                  migration.sql                snapshot.json
                                  (apply artifact)             (next diff + DAG)

migrate: migration.sql folders --lexical sort--> remove names already in DB ledger
                                               --> apply every missing SQL migration
                                               --> insert its folder name in the ledger
```

The snapshot is a desired-schema checkpoint used offline by `generate` and `check`; it is not an
introspection of a live database. The SQL file is the deployment artifact. The database-side
migration table is a separate applied-migration ledger. Current v1 source does not reconcile those
three truths automatically.

## 1. Release status: "v1" is still a prerelease

As of 2026-08-18, npm's `latest` tags still select `drizzle-kit@0.31.10` and
`drizzle-orm@0.45.2`. The `rc` tag selects `1.0.0-rc.4`; there is also an unpublished-as-GitHub-release
`rc5` canary (`drizzle-kit@1.0.0-rc.5-ab785fc`, `drizzle-orm@1.0.0-rc.5-169397b`). These values are
in the packages' first-party [`dist-tags`](https://registry.npmjs.org/drizzle-kit) and the official
upgrade page still tells users to install `@rc` ([upgrade guide](https://orm.drizzle.team/docs/upgrade-v1)).
The official roadmap still leaves **V1 RELEASE STREAM** unchecked
([roadmap source](https://github.com/drizzle-team/drizzle-orm-docs/blob/f0262250dd0c28c7e256703e5a81e829b2a83d73/src/data/roadmap.md#L15-L26)).

Therefore:

- the old journal workflow has been removed **inside the v1 prerelease line**;
- it has not yet been removed from the default `drizzle-kit` installed by an unqualified
  `npm install drizzle-kit`;
- details described from source below remain prerelease contracts until v1 GA.

## 2. What changed from v0

| Concern | v0 / current npm `latest` | v1 RC |
|---|---|---|
| Files | top-level `0000_name.sql`; snapshots under `meta/` | one timestamped folder per migration |
| Shared index | `meta/_journal.json` appended on every generation | none |
| History relation | `prevId: string` | `prevIds: string[]` |
| Snapshot shape | nested catalogue maps (`tables`, nested `columns`, etc.) | flat tagged `ddl` entity list |
| Fork policy | reject siblings sharing one parent as a collision | represent a DAG and statically check branch commutativity |
| Apply identity | journal timestamp / old `created_at` behavior | full migration folder `name` |

### 2.1 v0 was a journal plus a linked list

The v0 folder looked like this:

```text
drizzle/
├── meta/
│   ├── _journal.json
│   ├── 0000_snapshot.json
│   └── 0001_snapshot.json
├── 0000_public_electro.sql
└── 0001_perpetual_sebastian_shaw.sql
```

The journal stored `idx`, snapshot `version`, millisecond `when`, migration `tag`, and
`breakpoints`; generation appended to it after writing the next numbered snapshot
([v0 journal type](https://github.com/drizzle-team/drizzle-orm/blob/4aa6ecfee4b4728dadf6f77f071a149878a3c6c0/drizzle-kit/src/utils.ts#L63-L80),
[v0 writer](https://github.com/drizzle-team/drizzle-orm/blob/4aa6ecfee4b4728dadf6f77f071a149878a3c6c0/drizzle-kit/src/cli/commands/migrate.ts#L1395-L1439)).
Each Postgres snapshot had an `id` and singular `prevId`, along with nested tables, columns,
indexes, constraints, and `_meta` rename maps
([v0 Postgres validator](https://github.com/drizzle-team/drizzle-orm/blob/4aa6ecfee4b4728dadf6f77f071a149878a3c6c0/drizzle-kit/src/serializer/pgSchema.ts#L343-L449)).
The generator sorted snapshots, selected the last one, and made its `id` the new snapshot's parent
([v0 base selection](https://github.com/drizzle-team/drizzle-orm/blob/4aa6ecfee4b4728dadf6f77f071a149878a3c6c0/drizzle-kit/src/migrationPreparator.ts#L184-L209)).

That design serialised a workflow which Git does not serialise. Two feature branches both changed
the journal and both produced a child of the same snapshot. Drizzle treated the siblings as a race
condition even when the changes were independent. The v3 design discussion calls the shared format
"git incompatible" and explains that the latest snapshot can itself be only one side of a fork
([discussion #2832](https://github.com/drizzle-team/drizzle-orm/discussions/2832)).

### 2.2 v1 removes the shared conflict point

The v1 layout is:

```text
drizzle/
├── 20260818112030_add_users/
│   ├── migration.sql
│   └── snapshot.json
└── 20260818114302_add_posts/
    ├── migration.sql
    └── snapshot.json
```

There is no `_journal.json`. A migration owns its SQL and snapshot in one directory, whose suffix
is either `--name` or a generated adjective/name pair. Current code creates exactly those two files
([writer](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/generate-common.ts#L56-L87),
[name generation](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/utils/words.ts#L1-L9)).
The official change notes say this removes journal merge conflicts, makes a conflicted migration
removable by deleting its folder, and removes `drizzle-kit drop`
([v0→v1 changes](https://orm.drizzle.team/docs/v0-v1-changes)). Deleting a folder only changes
local history; it does not reverse SQL already recorded as applied in a database.

## 3. End-to-end generation

### 3.1 Inputs and ordinary base selection

`drizzle-kit generate` requires a dialect and one or more TypeScript/JavaScript schema paths; `out`
defaults to `./drizzle`. It does not require database credentials. It loads the declared schema into
the dialect's DDL representation and discovers snapshots by looking for
`<out>/<child>/snapshot.json`, then lexically sorting those paths
([official generate contract](https://orm.drizzle.team/docs/drizzle-kit-generate),
[folder scan](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/utils/utils-node.ts#L111-L123)).

For a linear history, the base is the lexically last snapshot. With no history, the base is an empty
"origin" snapshot whose id is the all-zero UUID. `generate` compares that base DDL with the target
DDL produced from the current schema
([Postgres preparation](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/serializer.ts#L13-L97),
[origin](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/utils/index.ts#L4-L5)).
Forks alter that base-selection step; section 5 covers it.

### 3.2 The v1 DDL snapshot

A current Postgres snapshot is conceptually:

```jsonc
{
  "version": "8",
  "dialect": "postgres",
  "id": "<new random UUID>",
  "prevIds": ["<parent snapshot UUID>", "<optional second parent>"],
  "ddl": [
    { "entityType": "tables", "schema": "public", "name": "users", "isRlsEnabled": false },
    { "entityType": "columns", "schema": "public", "table": "users", "name": "id", "type": "integer" }
  ],
  "renames": ["public.users.full_name->public.users.display_name"]
}
```

The exact validator is `{version, dialect, id, prevIds, ddl, renames}`; current versions are
Postgres 8, MySQL 6, SQLite/Turso 7, MSSQL 2, Cockroach 1, and SingleStore 2
([Postgres snapshot](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/snapshot.ts#L532-L562)).
"DDL snapshot" means that tables, columns, indexes, primary keys, foreign keys, unique constraints,
checks, policies, views, roles, and other objects are peer rows in a flat tagged list, rather than
children in a serialised database-shaped tree
([Postgres DDL definition](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/ddl.ts#L4-L160)).

There are two distinct notions of identity:

1. `snapshot.id` is a random UUID identifying a **history node**. `prevIds` are graph edges.
2. A DDL entity has no immutable UUID. Its lookup key is
   `schema:table:name:entityType` (empty segments where inapplicable)
   ([composite key](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/dialect.ts#L143-L150)).

That second choice is why a name change is inherently ambiguous: by identity, the old row vanished
and a new row appeared. `renames` records the human's decision as strings after the diff, but normal
Postgres generation reconstructs the next base from `ddl`; current source does not use old
`renames` entries as stable entity IDs.

### 3.3 Diff algorithm

The generic engine maps each old and new entity list by the composite key. An old-only key becomes
a drop, a new-only key becomes a create, and a shared key with changed non-identity fields becomes
an alteration
([generic diff](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/dialect.ts#L773-L872)).

Postgres then resolves entity kinds in dependency-aware stages. In broad terms:

1. set-diff one entity kind;
2. pass its creates and drops to a rename/create resolver;
3. mutate the old DDL for accepted renames, including dependent references;
4. compute remaining property alterations;
5. construct an ordered list of typed JSON DDL statements;
6. render each typed statement through exactly one dialect SQL converter
   ([Postgres diff pipeline](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/diff.ts#L83-L338),
   [SQL conversion](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/convertor.ts#L1102-L1125)).

This is an offline desired-state diff. `generate` neither introspects the database nor detects drift
between a snapshot, edited SQL, and actual database state.

### 3.4 Rename-or-create: TTY prompts and non-TTY hints

A resolver does nothing special unless the same entity kind has at least one create **and** at least
one drop. When both exist, each created entity must be classified as either genuinely new or a
rename/move from one of the still-unmatched deleted entities. A rename consumes its source so it
cannot be paired twice
([resolver](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/prompts.ts#L34-L205)).

The CLI is interactive only when `output === "text"` and stdin is a TTY. JSON output is always
non-interactive. In a TTY it presents the create option and possible deleted sources. Outside a TTY,
it returns `status: "missing_hints"` (exit code 2) instead of guessing. The caller retries with
`--hints` or `--hints-file`
([output modes](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/OUTPUT_MODES.md#L1-L56),
[hint contract](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/HINTS.md#L1-L82)).

Example request and the two possible replies:

```jsonc
// generate --output json
{
  "status": "missing_hints",
  "unresolved": [
    { "type": "rename_or_create", "kind": "column", "entity": ["public", "users", "display_name"] }
  ]
}

// rename
[{ "type": "rename", "kind": "column",
   "from": ["public", "users", "full_name"],
   "to": ["public", "users", "display_name"] }]

// or independent create (the old column remains a separate drop)
[{ "type": "create", "kind": "column",
   "entity": ["public", "users", "display_name"] }]
```

The wider hint vocabulary also has `confirm_data_loss`, but current `generate` handlers wire only
rename/create resolvers. Runtime `non_empty` warnings require a live target and belong to `push`;
offline generation cannot probe whether a table has rows
([hint types](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/hints.ts#L54-L98),
[Postgres generate wiring](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/generate-postgres.ts#L56-L101)).

### 3.5 Outputs

If there is a non-empty diff, Drizzle writes the new target snapshot and rendered SQL in a new
migration folder. Statements are separated with `--> statement-breakpoint` when breakpoints are
enabled. The v1 converter emits direct DDL such as `CREATE TABLE`, `DROP TABLE`, and `ALTER TABLE`
without v0-style `IF NOT EXISTS` guards
([writer](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/generate-common.ts#L56-L87),
[Postgres table SQL](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/convertor.ts#L114-L235)).

With no SQL statements, ordinary generation returns `no_changes` and writes neither file. A
`--custom` migration is the exception: it writes a SQL placeholder and a snapshot whose DDL is a
copy of the previous state. Consequently, hand-written **schema** DDL in that SQL is not reflected
in the snapshot unless some later process repairs the history; custom data-only SQL does not have
that mismatch
([custom snapshot and writer](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/serializer.ts#L83-L97),
[custom output](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/generate-common.ts#L76-L87)).

## 4. Forks, `prevIds`, `check`, and branch merging

Suppose two branches generated from snapshot `A`:

```text
      B   (leaf: adds users.email)
     /
A --
     \
      C   (leaf: adds posts.title)
```

The two snapshots both name `A` in `prevIds`. After Git merges the folders, `B` and `C` are both
open leaves: their IDs are not referenced as a parent by any other snapshot. A later merge node is:

```text
B --\
     D   prevIds = [B.id, C.id]
C --/
```

### 4.1 What `drizzle-kit check` actually checks

`check` first validates that every snapshot is parseable and on the current per-dialect snapshot
version. For supported dialects it then builds the graph from `id`/`prevIds`, locates forks, diffs
the fork parent snapshot against each descendant leaf, and converts those deltas to typed DDL
statements
([check handler](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/check.ts#L138-L239),
[graph engine](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/commutativity/engine.ts#L283-L449)).

"Commutative" here is a **static rule-table judgment**, not execution of `B; C` and `C; B` in two
databases. Each statement has resource footprints (schema/table/column/etc.) and a configured list
of statement types with which it conflicts. The engine reports an intersection between one
branch's touched footprints and the other's conflict footprints
([engine intersection](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/commutativity/engine.ts#L451-L526),
[Postgres rule map](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/commutativity.ts#L42-L178)).
For example, altering the same column conflicts; independent changes can commute. The official
design discussion describes the intended workflow and manual conflict recovery
([discussion #5005](https://github.com/drizzle-team/drizzle-orm/discussions/5005)).

Standalone `check` is read-only: it reports success or a conflict tree. Both `drizzle-kit generate`
and `drizzle-kit migrate` invoke the same check before continuing
([generate invocation](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/schema.ts#L94-L129),
[migrate invocation](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/schema.ts#L354-L380)).
Calling a `drizzle-orm` runtime `migrate()` directly does not run Drizzle Kit's checker.

### 4.2 How a safe fork becomes the next generation base

When the whole open history is conflict-free and has multiple leaves, the checker:

1. finds the open leaves and their lowest common ancestor (LCA);
2. computes `diff(LCA, leaf)` for every leaf;
3. de-duplicates identical typed statements;
4. returns the LCA snapshot, composed statements, and all leaf IDs to `generate`
   ([commutative merge construction](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/commutativity/engine.ts#L400-L449)).

The generator replays those statements into the LCA snapshot in memory, producing the combined
previous DDL. It diffs the current TypeScript target against that combined DDL, and the new
snapshot's `prevIds` are all open leaf IDs
([base materialisation](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/serializer.ts#L25-L81)).
The new migration SQL therefore contains only changes **after** the two existing branch migrations;
it does not repeat their SQL.

There is a subtle no-op case: if the TypeScript schema already equals the composed state of `B` and
`C`, the final diff is empty, and the common writer returns `no_changes` before creating `D`.
`check` itself does not create `D` either. The fork remains open until a later non-empty (or custom)
migration writes a multi-parent snapshot
([no-change early return](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/generate-common.ts#L54-L68)).

### 4.3 Conflict and escape-hatch behavior

Without an override, any footprint conflict aborts `check`, `generate`, or `drizzle-kit migrate`.
The intended remedy is manual: choose an ordering, remove/regenerate the losing branch's conflicted
migrations, and review the result.

`--ignore-conflicts` bypasses this. Official docs explicitly say that needing it probably means the
checker is wrong and ask users to report the case
([generate docs](https://orm.drizzle.team/docs/drizzle-kit-generate)). Current source preserves all
open leaf IDs as the new `prevIds`, but—because conflicting deltas cannot be composed safely—uses
the lexically latest snapshot as the actual diff base
([override result](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/check.ts#L187-L235),
[base choice](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/serializer.ts#L25-L75)).
That is an explicit unsafe escape hatch, not an automatic conflict merge.

### 4.4 Dialect coverage in current source

At `rc.4` and the `rc5` canary source commit, commutativity engines are wired only for PostgreSQL,
MySQL, SQLite, and Turso
([dialect registry](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/commutativity/index.ts#L1-L22)).
The official roadmap marks both "SQLite, MSSQL" as complete, but MSSQL is not present in that
registry. CockroachDB and SingleStore are also absent. Treat MSSQL commutativity as a documentation /
implementation mismatch, not as shipped behavior.

## 5. Applying migrations: ordering and the database ledger

Snapshots and their DAG are not used by the ORM migrator. Application follows a simpler algorithm:

1. enumerate child folders containing `migration.sql`;
2. sort by the full folder name lexicographically;
3. parse the first 14 characters as the legacy `created_at` timestamp and hash the complete SQL
   file with SHA-256;
4. load all rows from the database ledger;
5. keep every local migration whose full `name` is absent from the ledger;
6. apply those missing migrations in the already-sorted order and insert one ledger row per
   migration
   ([file reader](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-orm/src/migrator.ts#L48-L87),
   [pending selector](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-orm/src/migrator.utils.ts#L14-L24)).

This is why v1 can apply an older-timestamped migration merged later: it no longer asks whether its
timestamp is newer than the last applied migration; it asks whether its exact folder name is
missing. Among migrations missing in the current run, lexical folder order still determines
execution order. Commutativity checking is what is supposed to make independently generated sibling
migrations safe under that order.

For Postgres the current ledger defaults to `drizzle.__drizzle_migrations` and has:

| Column | Purpose |
|---|---|
| `id serial primary key` | insertion identity |
| `hash text not null` | SHA-256 of `migration.sql` at application time |
| `created_at bigint` | 14-digit folder prefix converted to milliseconds; retained for compatibility |
| `name text` | full migration folder name; current applied/not-applied identity |
| `applied_at timestamptz default now()` | actual application time; `NULL` for backfilled v0 rows |

The implementation creates, reads, and inserts those fields here
([Postgres migrator](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-orm/src/pg-core/async/session.ts#L290-L346)).
It stores `hash`, but the current pending selector compares only `name`; it does not reject an
already-applied migration whose SQL file was later edited. `applied_at` is also not used to select or
order pending migrations. The ledger contains no snapshot ID or `prevIds`, so it is a set of applied
folder names, not a persisted copy of the snapshot DAG.

Transaction/batch details vary by driver. The normal async Postgres path wraps all pending
migrations and ledger inserts in a transaction; proxy/HTTP and other dialect paths use their driver
facilities. The invariant to rely on is name-based selection, not identical transaction semantics
across every driver.

### 5.1 Automatic ledger upgrade is separate from `drizzle-kit up`

On first `migrate` against the old three-column ledger (`id`, `hash`, `created_at`), Drizzle detects
the schema shape, adds `name` and `applied_at`, and backfills names by second-level timestamp, using
hash as a tiebreaker/fallback. It refuses the upgrade if an existing DB row cannot be matched to a
local migration
([Postgres ledger upgrader](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-orm/src/up-migrations/pg.ts#L25-L108),
[version detection](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-orm/src/up-migrations/pg.ts#L135-L193)).
The ledger version is inferred from its columns; despite an early beta changelog showing a
`version` column, current source does not create one.

## 6. `drizzle-kit up`: local folder and snapshot conversion

`drizzle-kit up` is an on-disk migration-history upgrader, not a database migration runner. It needs
`dialect` and `out`, not DB credentials.

For a v0 history it:

1. reads `meta/_journal.json`;
2. creates one timestamp/name directory for each journal entry;
3. copies that entry's old snapshot and SQL into `snapshot.json` and `migration.sql`;
4. deletes the old top-level SQL;
5. removes the old `meta` directory;
6. validates and rewrites every non-current snapshot to the latest dialect format
   ([folder conversion](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/utils.ts#L968-L1020),
   [Postgres up handler](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/up-postgres.ts#L8-L34)).

For Postgres, v7→v8 flattens the catalogue into DDL entities, preserves the snapshot `id`, wraps
the old singular `prevId` in `prevIds`, and reconstructs `renames` from `_meta`. Older versions are
chained through v5, v6, and v7; versions below 4 throw and are not upgradeable by this code
([Postgres conversion](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/versions.ts#L22-L30),
[v8 result](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/versions.ts#L342-L358),
[minimum version](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/versions.ts#L407-L421)).
The converter computes advisory name-change `hints`, but the current `upPgHandler` discards them and
writes only `snapshot`.

Because `up` removes and rewrites local history, it should be run on a clean, committed tree and its
result reviewed. The database ledger is upgraded later by `migrate`, as described above.

## 7. What is shipped, what is only planned, and what remains uncertain

| Item | Status on 2026-08-18 | Evidence / qualification |
|---|---|---|
| v3 per-migration folders, no journal | in v1 RC | docs and writer source agree |
| Flat DDL snapshots | in v1 RC | current per-dialect snapshot validators |
| Snapshot DAG with `prevIds` | in v1 RC | source and beta.16/rc.2 release notes |
| `check` + automatic safe-base composition | in v1 RC for PG, MySQL, SQLite/Turso | source registry; not every dialect |
| Non-TTY structured hints | in v1 RC | `--output json`, `--hints`, `--hints-file` source and contract docs |
| Apply all missing folders by full name | in v1 RC | ORM migrator source |
| v1 GA | **not shipped** | npm `latest` remains 0.x; roadmap release item unchecked |
| Down migrations / better rollbacks | **roadmap** | explicitly unchecked on the official roadmap |
| `migration.ts` with `up`/`down` | **discussion proposal, not current implementation** | #2832 proposed it; writer currently emits only `migration.sql` |
| Database-aware `check --db` and GitHub auto-fix action | **discussion TODO** | still listed as TODO in #2832; no corresponding current command path found |
| Snapshot-less `generate --from-db` | **open user proposal, not accepted roadmap** | [open issue #5528](https://github.com/drizzle-team/drizzle-orm/issues/5528) has no maintainer response |
| MSSQL commutativity | **uncertain / docs mismatch** | roadmap checks it off; current registry does not wire it |

The v1 roadmap marks the broad folder and commutativity work complete, but that should not be read
as a stable compatibility guarantee while the package remains on RC/canary tags. The `rc5` canary
did not materially replace the migration architecture documented here, but it is not a tagged
GitHub release and should not be treated as GA.

## 8. Lessons relevant to Aureline

These are observations, not decisions:

1. **A per-migration snapshot avoids a shared Git hot spot.** Removing the journal solves the
   mechanical merge conflict; it does not by itself solve semantic branch conflicts.
2. **History identity and schema-entity identity are different problems.** UUID snapshot nodes plus
   multi-parent edges model Git-like history. Name-derived entity identity still needs a human or
   explicit hint to distinguish rename from create+drop.
3. **A DAG needs a merge-state algorithm, not just `prevIds`.** Drizzle composes conflict-free leaf
   deltas over an LCA. Merely attaching all leaves while diffing one latest snapshot is not enough;
   that is exactly the risk behind `--ignore-conflicts` and uncovered dialects.
4. **Static commutativity is a maintained semantic rule set.** It is fast and database-free, but
   correctness depends on exhaustive statement/resource footprints and regression tests; Drizzle's
   beta changelogs record fixes for missed leaves and index-scope false positives
   ([beta.19](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-beta.19),
   [beta.22](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-beta.22)).
5. **Generation history and deployment history are separate protocols.** Drizzle's snapshot DAG
   proves what generation believed, while the DB ledger remembers folder names. Neither proves the
   live schema equals the snapshot.
6. **Arbitrary SQL creates an honesty boundary.** A custom migration can safely carry data changes,
   but hand-written DDL makes the stored desired-state snapshot stale unless the tool also models or
   introspects that DDL.
7. **A no-op merge needs an explicit product decision.** Drizzle does not persist a merge node when
   the composed branch state already equals the declared schema. A system that wants a closed,
   reviewable DAG may need a metadata-only merge artifact.
8. **Format upgrades are product behavior.** Snapshot formats require validators, conversion ladders,
   minimum supported versions, and reviewable loss reporting from the first breaking change.

## Primary-source index

- Official docs: [v0→v1 changes](https://orm.drizzle.team/docs/v0-v1-changes),
  [upgrade guide](https://orm.drizzle.team/docs/upgrade-v1),
  [`generate`](https://orm.drizzle.team/docs/drizzle-kit-generate),
  [`check`](https://orm.drizzle.team/docs/drizzle-kit-check),
  [`up`](https://orm.drizzle.team/docs/drizzle-kit-up),
  [roadmap](https://orm.drizzle.team/roadmap)
- Official releases: [`beta.16`](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-beta.16),
  [`beta.22`](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-beta.22),
  [`rc.2`](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-rc.2),
  [`rc.4`](https://github.com/drizzle-team/drizzle-orm/releases/tag/v1.0.0-rc.4)
- First-party design discussions: [folder structure #2832](https://github.com/drizzle-team/drizzle-orm/discussions/2832),
  [commutative migrations #5005](https://github.com/drizzle-team/drizzle-orm/discussions/5005)
- Pinned implementation: [`drizzle-kit/src` at rc.4](https://github.com/drizzle-team/drizzle-orm/tree/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src),
  [`drizzle-orm/src/migrator.ts` at rc.4](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-orm/src/migrator.ts),
  [`rc5` canary source commit](https://github.com/drizzle-team/drizzle-orm/commit/ab785fcd99710d6d136ffbfd121b7aeb96e4d51d)
