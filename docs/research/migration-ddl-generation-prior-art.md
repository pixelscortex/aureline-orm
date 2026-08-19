# Migration DDL generation prior art

> Researched 2026-08-18 against Drizzle ORM v1 beta source at commit
> [`748058e`](https://github.com/drizzle-team/drizzle-orm/tree/748058e837d9c4247330e3d45580cbdae52bffda)
> and Prisma Engines `main` at commit
> [`561d7b4`](https://github.com/prisma/prisma-engines/tree/561d7b42579a2459cc8edf3788918b626c640023).
> This note asks one narrow question: after a schema diff is known, how do mature migration
> engines choose create, alter, replace, and remove DDL?

## Conclusion

Drizzle and Prisma do **not** make ordinary generated migrations broadly idempotent. Their normal
table and column statements are strict: create an entity that the prior state says is absent, alter
an entity that the prior state says exists, and drop an entity that the prior state says was
removed. Neither generator routinely adds `IF NOT EXISTS` or `IF EXISTS` to those statements.

The intelligence sits one layer above the SQL renderer:

```text
previous state + desired state
            -> semantic diff
            -> ordered, classified migration steps
            -> dialect-specific strict DDL
```

The renderer does not hide a wrong starting state. If a supposedly new table already exists, or a
supposedly existing column is missing, execution should fail and expose drift or a broken history.

The reusable rule for Aureline is therefore:

1. Model the change before rendering text.
2. Use a native in-place mutation when it faithfully expresses that change.
3. Use replacement only when the database lacks a faithful mutation, and classify its consequence.
4. Use remove-plus-create only as the last, explicitly destructive fallback.
5. Do not add existence guards to ordinary migration DDL. Reserve guards for deliberately
   idempotent infrastructure bootstrap, if such a use case actually appears.

## Drizzle v1

### Diff first, render second

The v1 `generate` documentation describes the pipeline explicitly: read the declared schema,
compose a JSON snapshot, compare it with previous migration snapshots, generate SQL from the
difference, then persist `migration.sql` and `snapshot.json` together
([official `generate` documentation](https://orm.drizzle.team/docs/drizzle-kit-generate)).

In the PostgreSQL v1 implementation, created and deleted tables become distinct `create_table` and
`drop_table` statements, while column creates/deletes and recognized renames become their own
steps
([diff construction](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/diff.ts#L708-L725),
[created-table step](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/diff.ts#L1100-L1108)).

### Strict ordinary DDL

The PostgreSQL renderer emits:

- plain `CREATE TABLE`, without `IF NOT EXISTS`;
- plain `DROP TABLE`, without `IF EXISTS`;
- `ALTER TABLE ... ADD COLUMN` and `ALTER TABLE ... DROP COLUMN`, without guards;
- targeted `ALTER TABLE ... ALTER COLUMN` statements for supported mutations; and
- drop-plus-add for the narrower cases represented as `recreate_column`.

See the v1
[`create_table` and `drop_table` converters](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/convertor.ts#L114-L236)
and the
[`add`, `drop`, `recreate`, and `alter` column converters](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/dialects/postgres/convertor.ts#L252-L388).
The official v1 example likewise shows unguarded `CREATE TABLE`
([generated migration example](https://orm.drizzle.team/docs/drizzle-kit-generate)).

This is a deliberate improvement over older Drizzle output. In the design discussion leading to
the migration rewrite, the maintainer called out the failure mode of `IF NOT EXISTS`: a migration
can report success even though a same-named table already exists and may have the wrong shape
([Drizzle discussion #2624](https://github.com/drizzle-team/drizzle-orm/discussions/2624)).

### The file is an execution plan, not a reconciliation script

Drizzle joins the already-rendered statements and writes them directly to `migration.sql`; it does
not add a universal transaction or reconciliation wrapper at generation time
([migration writer](https://github.com/drizzle-team/drizzle-orm/blob/748058e837d9c4247330e3d45580cbdae52bffda/drizzle-kit/src/cli/commands/generate-common.ts#L56-L87)).
Applying and recording migrations is a separate responsibility with a database-side ledger
([official `migrate` documentation](https://orm.drizzle.team/docs/drizzle-kit-migrate)).

## Prisma Migrate

### A typed migration-step planner

Prisma's SQL schema differ constructs ordered step variants such as `CreateTable`, `DropTable`,
`AlterTable`, and `RedefineTables` before any dialect renderer produces SQL
([step calculation and ordering](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/schema-engine/connectors/sql-schema-connector/src/sql_schema_differ.rs#L23-L46)).

For column type changes, Prisma explicitly classifies the conversion:

- `SafeCast` -> alter the column;
- `RiskyCast` -> alter the column, with diagnostics available to the caller; and
- `NotCastable` -> drop and recreate the column.

That decision is visible in the
[`ColumnTypeChange` branch](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/schema-engine/connectors/sql-schema-connector/src/sql_schema_differ.rs#L185-L224).
The PostgreSQL renderer then converts the typed changes into add, drop, alter, or drop-plus-add
clauses
([PostgreSQL table-change renderer](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/schema-engine/connectors/sql-schema-connector/src/flavour/postgres/renderer.rs#L244-L318)).

### Strict tables, with narrow guarded exceptions

Prisma's ordinary PostgreSQL table renderer emits plain `CREATE TABLE` and plain `DROP TABLE`, with
no existence guards
([create and drop renderers](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/schema-engine/connectors/sql-schema-connector/src/flavour/postgres/renderer.rs#L400-L457)).
The generated examples in Prisma's getting-started guide use plain `CREATE TABLE` for an initial
migration and `ALTER TABLE ... ADD COLUMN` for a subsequent one
([official getting-started guide](https://www.prisma.io/docs/orm/prisma-migrate/getting-started)).

Prisma does use `IF NOT EXISTS` selectively for resources whose bootstrap is intentionally shared,
such as a PostgreSQL namespace in the same renderer. This is a resource-specific policy, not a
general migration-idempotency policy
([namespace renderer](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/schema-engine/connectors/sql-schema-connector/src/flavour/postgres/renderer.rs#L393-L397)).

### Diagnostics are part of the artifact

Prisma writes destructive and unexecutable diagnostics into a comment at the top of the generated
migration, followed by labelled migration steps
([script renderer](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/schema-engine/connectors/sql-schema-connector/src/apply_migration.rs#L36-L110)).
The official customization guide shows the consequence of a rename it cannot infer: generated
drop-plus-add SQL must be reviewed and changed to native rename SQL to retain data
([customizing migrations](https://docs.prisma.io/docs/orm/prisma-migrate/workflows/customizing-migrations)).

Prisma also treats edited or missing applied migrations and out-of-band database changes as
history conflicts or drift instead of allowing guarded DDL to paper over them
([official troubleshooting guide](https://www.prisma.io/docs/orm/prisma-migrate/workflows/troubleshooting)).

### Transaction handling is dialect-specific

Prisma's script writer asks the dialect renderer whether it wants begin/commit wrappers; it does
not impose one universal policy
([transaction hooks in the script renderer](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/schema-engine/connectors/sql-schema-connector/src/apply_migration.rs#L74-L108)).
For example, the SQL Server renderer supplies a `BEGIN TRY` / transaction / rollback structure
([SQL Server renderer](https://github.com/prisma/prisma-engines/blob/561d7b42579a2459cc8edf3788918b626c640023/schema-engine/connectors/sql-schema-connector/src/flavour/mssql/renderer.rs#L451-L470)),
while the PostgreSQL renderer does not override those hooks. Transaction ownership therefore
belongs to the database/dialect policy, not to a generic migration text template.

## What Aureline should copy

### A semantic operation matrix

Aureline should diff flat snapshot entities into a structured `Migration Plan` before rendering
SurrealQL. For every entity property, the planner should have an explicit capability entry:

| Change | Preferred operation | Fallback | Required classification |
|---|---|---|---|
| Entity added | strict `DEFINE` | none | ordinary |
| Supported in-place mutation | `ALTER` | none | safe or invalidating, depending on property |
| Unsupported in-place mutation but faithful replacement exists | `DEFINE ... OVERWRITE` | none | prove and document preservation semantics |
| Entity removed | strict `REMOVE` | none | data loss or data invalidation |
| Mutation requiring recreation | `REMOVE` + `DEFINE` | manual migration if preservation is needed | destructive |

`OVERWRITE` is therefore not the normal update verb and not an idempotency switch. It is one
database-specific operation available only for changes whose semantics are known. In the initial
table slice, SurrealDB 3.x can alter the modeled table schema mode and field type, so those changes
should use `ALTER`; no `OVERWRITE` case is yet justified.

### Strict execution assumptions

For ordinary table and field migrations:

- do not emit `IF NOT EXISTS` for additions;
- do not emit `IF EXISTS` for modifications or removals;
- let an unexpected starting state fail;
- keep data-loss and data-invalidation findings as structured generation output, and also render
  them into the human-reviewable migration file; and
- leave transaction wrapping to the later SurrealDB apply design, after its actual atomicity and
  client behavior have been verified.

This gives Aureline a smart planner and an honest script: intelligence determines the exact
operation, while strict DDL detects when reality no longer matches the history that justified it.
