# Offline migration generation

`aureline_migration::generate(&checked, previous.as_ref())` accepts a generation-safe Checked Program and an optional prior `Snapshot`. It returns a classified `MigrationPlan`, a strict SurrealQL script, and the next snapshot. `Snapshot::from_json` validates the complete version-1 input before it can become a comparison base; `to_json` emits canonical flat entities. Record targets use stable exact names rather than compilation-local arena IDs.

The target is pinned to SurrealDB 3.2.0. See [sized-set enforcement](docs/adr/0001-surrealdb-3-2-set-enforcement.md) for supported recursive constraints and generation-blocking unions. The library performs no network or database operations.

The generation pipeline has three reviewable seams: `MigrationModel::lower` translates checked semantic facts into stable names, `MigrationPlan` compares the previous and current models in dependency-safe phases, and the renderer turns those operations into target DDL. `Snapshot::from_json` is the trust boundary for persisted history; it reparses and rechecks flat entities before they can become the comparison base. This means a snapshot can be inspected or rejected without allowing untrusted text to flow directly into generated SQL.

Callers inspect `plan.is_empty()` and write nothing for an unchanged schema. For a nonempty plan, the CLI slice (#53) owns the indivisible creation of:

```text
migrations/20260818143022_create_user/
├── migration.surql
└── snapshot.json
```

Folder naming uses a 14-digit UTC timestamp and a user-facing slug. Each snapshot has its own 16-character NanoID and a `prevIds` array containing zero or one predecessor; there is no journal. The library neither discovers branch histories nor records application state. Unknown snapshot versions, malformed entities, and unsupported target contracts fail rather than being upgraded or weakened.

Unsafe changes still generate. Structured plan warnings are repeated in the script header: table removal loses data, field-definition removal retains stored values but may invalidate later writes, and changes to `id` carry a record-key warning. Type mutations use `ALTER`; generated assertions are updated or removed with them. No operation uses existence guards, synthesizes defaults, or wraps transactions.

The first-migration contract is tested with `aurl_test!(source).compiles().ddl(expected)`. The prior-snapshot diff harness and real-database execution are intentionally deferred by #51 and #44 respectively; the operation matrix is independently reviewed.
