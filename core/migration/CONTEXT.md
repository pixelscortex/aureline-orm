# Migration Planning

Migration Planning is the context in which Aureline compares its last generated schema state with
the current checked declaration state and describes the database changes between them. It does not
observe or execute against a running database.

## Schema history

**Migration Model**:
The complete, target-neutral set of migration-supported schema facts lowered from a Checked Program.
It contains only facts needed to identify and compare database definitions.
_Avoid_: Checked Program, AST, live database schema

**Migration Snapshot**:
A versioned, committed record of a Migration Model after Aureline generated a migration, together
with its identity in generated schema history. It records what Aureline last generated, not what a
database has applied or currently contains.
_Avoid_: Journal, introspection result, applied-migration ledger

## Change planning

**Migration Plan**:
An ordered set of classified schema changes produced by comparing a prior Migration Snapshot with
the current Migration Model. Each change states its semantic operation and consequence before any
SurrealQL is rendered.
_Avoid_: Text diff, migration script, SQL string

**Migration Script**:
The reviewable SurrealQL rendering of a Migration Plan. It expresses the plan's expected transition
and is not a general-purpose reconciliation of arbitrary database state.
_Avoid_: Migration Snapshot, desired schema, applied-migration ledger
