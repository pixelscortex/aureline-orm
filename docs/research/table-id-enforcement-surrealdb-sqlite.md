# Typed and structured table IDs in SurrealDB 3.2 and SQLite

> Researched 2026-08-18 against SurrealDB 3.2 documentation, release notes, and
> tagged language tests, plus SQLite's official documentation. This is a findings note, not an
> Aureline specification.

## Conclusion

The direct answer for Aureline is a **SurrealDB** feature, not a SQLite workaround. `DEFINE TABLE`
does not carry an ID-key type, because SurrealDB defines fields separately. On SurrealDB 3.2 or
later, emit a `DEFINE FIELD id` immediately after the table definition:

```surql
DEFINE TABLE account SCHEMAFULL;
DEFINE FIELD id ON TABLE account TYPE int ASSERT id.id() > 0;
```

SurrealDB 3.2 made a typed `id` a first-class, write-time-enforced field. A write whose record-key
value cannot match the declared type fails; `DEFAULT` can generate a missing key, and `ASSERT` can
impose a narrower invariant such as positivity or valid ULID syntax. This behavior shipped in 3.2.0
and is explicitly called out as a behavior change in the [3.2 release
notes](https://surrealdb.com/releases/3.2#typed-id-enforced-at-write-time). The current [`DEFINE
FIELD` reference](https://surrealdb.com/docs/reference/query-language/statements/define/field#defining-a-type-for-the-id-field)
documents scalar, composite, `DEFAULT`, and `ASSERT` forms. Pair it with `DEFINE DATABASE ...
STRICT`, which rejects attempts to use undeclared tables/resources. The combination closes both
failure modes in the question: an unknown table cannot spring into existence, and a defined table
cannot receive a key of the wrong type.

SQLite is a separate database with separate syntax (`CREATE TABLE`, not `DEFINE`). It can enforce
integer, canonical text UUID, binary UUID, and composite IDs with `STRICT` tables plus `PRIMARY
KEY`/`CHECK`. It cannot install a database-wide rule that every table a future developer creates
must use one ID shape. That part must be enforced by restricting DDL to a trusted migration path and
checking generated schemas.

## 1. The apparent “string default” in SurrealDB

A SurrealDB record ID has two parts: a table name and a key. The key may be text, a number, a UUID,
an array, or an object. When `CREATE table` omits a key, SurrealDB's default generator happens to
produce a 20-character alphanumeric string. That is **not** the same as having a declared
`TYPE string` contract; absent an `id` definition, callers may still explicitly use other supported
key forms. See the [record-ID reference](https://surrealdb.com/docs/reference/query-language/language-primitives/data-types/record-ids).

`DEFINE TABLE ... TYPE NORMAL` is also unrelated to the key type. A table's `TYPE` distinguishes
`ANY`, `NORMAL`, and `RELATION`; fields are deliberately declared with separate `DEFINE FIELD`
statements. `SCHEMAFULL` closes the ordinary record shape, while `DEFINE FIELD id` supplies the key
contract. See [`DEFINE TABLE`](https://surrealdb.com/docs/reference/query-language/statements/define/table).

## 2. Native SurrealDB enforcement

### 2.1 Explicit positive integer keys

```surql
DEFINE TABLE account SCHEMAFULL;
DEFINE FIELD id ON TABLE account
  TYPE int
  ASSERT id.id() > 0;

CREATE account:42;       -- accepted
CREATE account:customer; -- rejected: key is a string, not an int
CREATE account:0;        -- rejected by ASSERT
CREATE account;          -- rejected unless a DEFAULT is supplied
```

The 3.2.0 tagged language tests verify that `TYPE int` retains an explicit integer, rejects a
non-integer key, and cannot synthesize an omitted integer key. They also verify the positive-key
assertion: [`id_write.surql`](https://github.com/surrealdb/surrealdb/blob/v3.2.0/language-tests/tests/language/statements/define/field/id_write.surql)
and [`id_assert.surql`](https://github.com/surrealdb/surrealdb/blob/v3.2.0/language-tests/tests/language/statements/define/field/id_assert.surql).
A local SurrealDB 3.2.4 verification was stricter than SQLite affinity: it accepted `ids:5`, rejected
the quoted string key ``ids:`6` ``, rejected `CREATE ids SET id = "7"`, and rejected an omitted key.

For database-generated numeric keys, a 3.2-compatible form is:

```surql
DEFINE SEQUENCE account_ids START 1;
DEFINE TABLE account SCHEMAFULL;
DEFINE FIELD id ON TABLE account
  TYPE int
  DEFAULT sequence::nextval('account_ids')
  ASSERT id.id() > 0;
```

SurrealDB sequences are globally unique, monotonically increasing generators, including in a
cluster. Sequence allocation is not rolled back with a failed transaction, so gaps are expected;
they are identities, not gap-free counters. See [`DEFINE
SEQUENCE`](https://surrealdb.com/docs/reference/query-language/statements/define/sequence).

### 2.2 UUID keys

```surql
DEFINE TABLE user SCHEMAFULL;
DEFINE FIELD id ON TABLE user
  TYPE uuid
  DEFAULT rand::uuid();

CREATE user; -- generated UUID
CREATE user:u"550e8400-e29b-41d4-a716-446655440000"; -- explicit typed UUID
CREATE user:notauuid; -- rejected
```

SurrealQL has a real `uuid` value type rather than treating UUIDs as formatted strings. Since
SurrealQL 2.0, a quoted string is not eagerly converted to UUID; use the `u"..."` literal, an
explicit `<uuid>` cast, the SDK's UUID value, or `rand::uuid()`. See [UUID
values](https://surrealdb.com/docs/reference/query-language/language-primitives/data-types/uuids).
In 3.2, `TYPE uuid` can auto-generate a UUID even without the explicit `DEFAULT` shown above, just
as `TYPE string` can auto-generate a string key; `TYPE int` cannot auto-generate. Declaring the
`DEFAULT` remains useful when Aureline wants the generator to be an explicit part of the schema
contract.

There is no separate `guid` type. In older SurrealDB versions `rand::guid()` was the name of what is
now `rand::id()`, which returns an alphanumeric **string**. Use `uuid`/`rand::uuid()` when “GUID”
means a UUID. See the [`rand` function reference](https://surrealdb.com/docs/reference/query-language/functions/database-functions/rand#rand-id).

### 2.3 ULID-shaped string keys

ULID is a format constraint on a string key, so combine `TYPE string` with generation and an
assertion:

```surql
DEFINE TABLE user SCHEMAFULL;
DEFINE FIELD id ON TABLE user
  TYPE string
  DEFAULT rand::ulid()
  ASSERT id.id().is_ulid();
```

This is the official 3.2 example. For an `id` assertion, `$value` is the whole record ID; `id.id()`
or `record::id($value)` extracts its key. The assertion is evaluated for generated,
default-supplied, and explicit IDs on create. See [`DEFINE FIELD`: `ASSERT` and `DEFAULT` on
`id`](https://surrealdb.com/docs/reference/query-language/statements/define/field#assert-and-default-on-id).

### 2.4 Composite keys

The key contract may describe structure, not merely one primitive:

```surql
DEFINE TABLE log SCHEMAFULL;
DEFINE FIELD id ON TABLE log
  TYPE [record, "info" | "warn" | "error", datetime];

CREATE log:[user:one, "info", time::now()]
  SET message = "Database started"; -- accepted

CREATE log:bad
  SET message = "Database started"; -- rejected
```

The field reference documents this exact tuple form and its coercion error. SurrealDB's 3.2.0
language tests additionally accept top-level `number`, `int`, `string`, `uuid`, `array`, `object`,
literal, and union key contracts, while rejecting types that cannot be record keys. Nested arrays
and objects can themselves carry typed structure. See
[`id_kind.surql`](https://github.com/surrealdb/surrealdb/blob/v3.2.0/language-tests/tests/language/statements/define/field/id_kind.surql).

### 2.5 Enforcement boundaries and upgrade behavior

- Typed-key enforcement requires SurrealDB **3.2.0 or later**. In 3.2, `CREATE`, `UPSERT`, and
  `INSERT` all pass through the typed-key check. A string key supplied to an `int` contract is a
  type mismatch rather than a valid integer key; this is a stored key-type contract, not merely an
  SDK annotation. The [3.2 release notes](https://surrealdb.com/releases/3.2#typed-id-enforced-at-write-time)
  and [tagged write tests](https://github.com/surrealdb/surrealdb/blob/v3.2.0/language-tests/tests/language/statements/define/field/id_write.surql)
  are the primary evidence.
- `DEFAULT` is evaluated when `CREATE` or `INSERT` supplies no ID and is coerced to the declared
  type. An explicit target/data ID wins. `DEFAULT ALWAYS` is forbidden because an explicit record
  ID must not be replaced. [`id_default.surql`](https://github.com/surrealdb/surrealdb/blob/v3.2.0/language-tests/tests/language/statements/define/field/id_default.surql)
  exercises all of these cases.
- `ASSERT` runs on creation, not update, because a record ID is immutable. It is skipped by
  `OPTION IMPORT`, whose purpose is to restore already-validated data while bypassing field
  validation. Treat import as a privileged escape hatch, not an ordinary write path. See the
  [`DEFINE FIELD` reference](https://surrealdb.com/docs/reference/query-language/statements/define/field#assert-and-default-on-id).
- `VALUE`, `REFERENCE`, `COMPUTED`, `READONLY`, `FLEXIBLE`, and key-incompatible `TYPE` forms are forbidden
  on `id`; use `TYPE`, plain `DEFAULT`, and `ASSERT` for this contract.
- The 3.2 change is not retroactive. Existing mismatched records remain readable, but future writes
  that do not match the declared ID type fail. The [3.2 upgrade warning](https://surrealdb.com/releases/3.2#typed-id-is-enforced-at-write-time)
  tells operators to audit existing `DEFINE FIELD id` declarations and actual keys before upgrade.

### 2.6 Preventing ad-hoc or malformed table definitions

Typed `id` prevents malformed **record writes**. Database strictness prevents CRUD from implicitly
creating an undeclared table:

```surql
DEFINE NAMESPACE application;
USE NS application;
DEFINE DATABASE app STRICT;
USE DB app;

DEFINE TABLE account SCHEMAFULL;
DEFINE FIELD id ON TABLE account TYPE int ASSERT id.id() > 0;

CREATE account:1; -- accepted
CREATE missing:1; -- rejected: table is not defined
```

Without database `STRICT`, `CREATE`, `INSERT`, and `UPSERT` may implicitly define missing resources;
with it, a resource must be defined before use. Strict database mode has existed since SurrealDB
3.0. See [`DEFINE DATABASE`: strict databases](https://surrealdb.com/docs/reference/query-language/statements/define/database#defining-a-strict-database)
and [`CREATE`: implicit statement behavior](https://surrealdb.com/docs/reference/query-language/statements/create#implicit-statement-behaviour).

This still does not create a meta-schema rule saying every deliberately defined future table must
have the same ID contract. Schema authority and migration validation remain relevant:

- `DEFINE TABLE` requires a root, namespace, or database system user with the `OWNER` or `EDITOR`
  role. Runtime record users are governed by table permissions instead. See [`DEFINE TABLE`
  requirements](https://surrealdb.com/docs/reference/query-language/statements/define/table#requirements)
  and [SurrealDB's RBAC overview](https://surrealdb.com/docs/learn/security/authentication/authentication#system-users).
- Reserve `OWNER`/`EDITOR` credentials for the migration path. Applications should connect as
  record users, or another role/access path that cannot issue schema definitions. A developer who
  can authenticate as a schema owner can also alter or remove the ID definition; no constraint can
  protect itself from its own administrator.
- The migration/compiler path should emit the table and its `id` field together and should reject
  a table declaration whose key contract cannot be represented. CI can inspect `INFO FOR DB` and
  `INFO FOR TABLE ...` to detect schema drift; [`INFO`](https://surrealdb.com/docs/reference/query-language/statements/info)
  returns stored table and field definitions.

## 3. Implications for Aureline

These are design consequences of the database facts, not a proposed source grammar.

1. **Target SurrealDB 3.2+ for native enforcement.** If Aureline supports an older server, it must
   diagnose that a declared ID contract cannot be guaranteed rather than silently emitting a
   decorative definition.
2. **Bootstrap an Aureline-managed database as `STRICT`.** This prevents a misspelled or undeclared
   table name in CRUD from silently creating a schemaless table. It complements, rather than
   replaces, each table's `SCHEMAFULL` and typed `id` definitions.
3. **Model the record-key contract as a first-class Table fact.** It should survive Resolution into
   the Checked Program and into the Migration Model. The SurrealQL renderer can lower that one fact
   to the database's two-statement representation: `DEFINE TABLE` plus `DEFINE FIELD id`.
4. **Keep the key type distinct from the full record identity.** `user:42` is a record ID for table
   `user`; `42` is its `int` key. Generated bindings can therefore preserve both the table brand and
   the key type instead of reducing every ID to `RecordId<Table, string>`.
5. **Represent supported structured keys faithfully.** SurrealDB accepts scalar, literal/union,
   array, and object key contracts, but not every general SurrealDB type is legal at the top level
   of a record key. Static Semantics should reject those before generation.
6. **Classify an ID-contract change as write compatibility work.** SurrealDB does not rewrite old
   keys when a definition changes. A migration from string to integer/UUID should require a data
   audit and an explicit policy for legacy records, not only a new `DEFINE FIELD` statement.
7. **Keep runtime and migration authority separate.** Database enforcement protects ordinary
   writes; restricted credentials and reviewed migrations protect the schema itself.

An economical future source spelling would keep identity where users look for the rest of a Table's
shape:

```aurl
table user schemafull {
  id uuid
}
```

`id` would be identity syntax with a **key** Semantic Type, not an ordinary stored scalar field; the
row's semantic identity remains `RecordId<user, uuid>`. The exact grammar and clauses for defaults
and assertions remain a separate design decision. The current scaffold only accepts newline-only
table bodies and `TableDecl` currently carries name plus schema mode, so this note describes the
next language capability rather than an implemented one.

The repository currently describes Aureline as a SurrealDB schema-and-query language. SQLite
should not leak into this contract unless SQLite becomes an intentional target.

## 4. SQLite comparison

SQLite has no `DEFINE` statement and no SurrealDB-style record ID. A table ID is simply one or more
columns selected as its `PRIMARY KEY`.

### 4.1 Why an ordinary declared type is insufficient

Ordinary SQLite uses dynamic typing. A declaration such as `id INT` gives the column INTEGER
*affinity*, which recommends conversion but does not normally forbid other storage classes. A
made-up declaration such as `id UUID` or `id GUID` does not validate a UUID; under SQLite's affinity
rules it falls through to NUMERIC affinity. See [Datatypes in
SQLite](https://www.sqlite.org/datatype3.html).

SQLite 3.37.0 added per-table `STRICT` mode. A strict column must be declared as one of `INT`,
`INTEGER`, `REAL`, `TEXT`, `BLOB`, or `ANY`, and a value that cannot be losslessly converted to the
declared type is rejected. Strict typing still performs conversion: text `'123'` in an `INT` column
becomes integer `123`; text `'customer'` fails. See [STRICT
tables](https://www.sqlite.org/stricttables.html).

### 4.2 Numeric IDs

For an explicit logical integer key, including rejection of omitted and arbitrary string IDs:

```sql
CREATE TABLE item (
  id INT PRIMARY KEY CHECK (id > 0),
  name TEXT NOT NULL
) STRICT;
```

`INT PRIMARY KEY` in a strict table is unique and implicitly not null, but is not a rowid alias.
SQLite may still losslessly coerce input such as `'123'` to integer. If the requirement is stronger
— reject a text-bound parameter even when its contents are numeric — use strict `ANY`, which
preserves the submitted storage class, and inspect it with `typeof()`:

```sql
CREATE TABLE item (
  id ANY PRIMARY KEY
    CHECK (typeof(id) = 'integer' AND id > 0),
  name TEXT NOT NULL
) STRICT;
```

The first form enforces the stored logical type. The second also enforces the input storage class.
The behavior follows from strict `ANY` preserving values without affinity conversion and
[`typeof()`](https://www.sqlite.org/lang_corefunc.html#typeof) reporting `integer`, `real`, `text`,
`blob`, or `null`.

The spelling `INTEGER PRIMARY KEY` is special. In an ordinary rowid table it aliases SQLite's
signed 64-bit rowid; `INT PRIMARY KEY` does not. The alias rejects nonnumeric text, but accepts a
losslessly convertible value and turns `NULL` into an automatically generated ID. Choose it when
that automatic rowid behavior is desired, not when an explicit ID is mandatory. See [`CREATE
TABLE`: ROWIDs and `INTEGER PRIMARY KEY`](https://www.sqlite.org/lang_createtable.html#rowids-and-the-integer-primary-key).

### 4.3 Canonical UUID text or 16-byte UUIDs

SQLite has no native UUID type. A canonical lower-case textual UUID can be checked entirely with
built-ins:

```sql
CREATE TABLE item (
  id TEXT PRIMARY KEY CHECK (
    length(id) = 36
    AND substr(id, 9, 1) = '-'
    AND substr(id, 14, 1) = '-'
    AND substr(id, 19, 1) = '-'
    AND substr(id, 24, 1) = '-'
    AND length(replace(id, '-', '')) = 32
    AND replace(id, '-', '') NOT GLOB '*[^0-9a-f]*'
  ),
  name TEXT NOT NULL
) STRICT;
```

Add `substr(id, 15, 1) = '4'` and `substr(id, 20, 1) GLOB '[89ab]'` if the contract specifically
requires RFC-style version-4/variant bits rather than any canonical 128-bit UUID spelling.

For a binary representation:

```sql
CREATE TABLE item (
  id BLOB PRIMARY KEY CHECK (length(id) = 16),
  name TEXT NOT NULL
) STRICT;
```

These checks rely on SQLite's deterministic built-in `length`, `substr`, `replace`, and `GLOB`
operations. SQLite's core does not provide a UUID datatype or a built-in regular-expression
validator. A custom deterministic `is_valid_uuid()` SQL function is possible, but every connection
that uses the schema must register it, and hardened `trusted_schema=OFF` connections will not allow
an application-defined function in a `CHECK` unless it has also been deliberately marked safe for
schema use. Built-ins are more portable. See [application-defined function security](https://www.sqlite.org/appfunc.html#security_implications).

### 4.4 Composite structure is often better than serialization

If an identity naturally consists of a tenant plus a local number, preserve those components:

```sql
CREATE TABLE tenant_item (
  tenant_id TEXT NOT NULL,
  local_id INT NOT NULL CHECK (local_id > 0),
  name TEXT NOT NULL,
  PRIMARY KEY (tenant_id, local_id)
) STRICT, WITHOUT ROWID;
```

SQLite supports composite primary keys. `WITHOUT ROWID` makes that composite key the table's
clustered key and enforces `NOT NULL` on every component; it is often useful for non-integer or
composite keys. See [`WITHOUT ROWID`](https://www.sqlite.org/withoutrowid.html).

If consumers also need one display/string form, derive it rather than accepting two independently
writable identities:

```sql
CREATE TABLE tenant_item (
  tenant_id TEXT NOT NULL,
  local_id INT NOT NULL CHECK (local_id > 0),
  id_text TEXT GENERATED ALWAYS AS (tenant_id || ':' || local_id) STORED UNIQUE,
  name TEXT NOT NULL,
  PRIMARY KEY (tenant_id, local_id)
) STRICT, WITHOUT ROWID;
```

A generated column may be `UNIQUE`, indexed, checked, or referenced by a foreign key, but SQLite
does not allow a generated column itself in the primary key. See [generated-column capabilities and
limitations](https://www.sqlite.org/gencol.html#capabilities).

### 4.5 `CHECK`, triggers, and foreign keys

- `CHECK` is evaluated on `INSERT` and `UPDATE`; a zero result fails, but `NULL` passes. Use
  `NOT NULL` unless a strict/`WITHOUT ROWID` primary key already supplies it. A `CHECK` cannot
  contain a subquery, and a privileged connection can disable checks with
  `PRAGMA ignore_check_constraints=ON`. See [`CHECK`
  constraints](https://www.sqlite.org/lang_createtable.html#check_constraints).
- A trigger can reject a row with `RAISE(ABORT, ...)` when validation needs a custom message or
  logic unsuitable for `CHECK`. It is not stronger schema protection: SQLite triggers are
  row-level `INSERT`/`UPDATE`/`DELETE` triggers, not DDL triggers, and a schema writer can drop them.
  See [`CREATE TRIGGER`](https://www.sqlite.org/lang_createtrigger.html).
- A foreign key enforces that a child key refers to an existing parent; it does not replace local
  type/shape checks. Define child components with the same strict types and checks, add `NOT NULL`
  if the relationship is mandatory, and use a composite foreign key for a composite ID. Foreign-key
  enforcement must be explicitly enabled on every connection with `PRAGMA foreign_keys=ON`; the
  historical default is off. See [SQLite foreign keys](https://www.sqlite.org/foreignkeys.html) and
  [composite foreign keys](https://www.sqlite.org/foreignkeys.html#fk_composite).

### 4.6 SQLite cannot impose a global “all table IDs look like this” rule

SQLite is an embedded library whose callers read and write the database file directly. It does not
implement `GRANT` or `REVOKE`; durable access control comes from operating-system file permissions.
See [features SQLite omits](https://www.sqlite.org/omitted.html) and [SQLite's serverless
architecture](https://www.sqlite.org/serverless.html).

The practical control stack is therefore:

1. Give normal application connections no DDL path. Open read-only consumers with
   `SQLITE_OPEN_READONLY`/`mode=ro` where possible.
2. On writable runtime connections, install `sqlite3_set_authorizer()` and return `SQLITE_DENY` for
   `SQLITE_CREATE_TABLE`, `SQLITE_CREATE_TEMP_TABLE`, `SQLITE_CREATE_VTABLE`,
   `SQLITE_ALTER_TABLE`, and all corresponding drop operations. The callback is per connection and
   disabled by default. See the [authorizer API](https://www.sqlite.org/c3ref/set_authorizer.html)
   and [action codes](https://www.sqlite.org/c3ref/c_alter_table.html).
3. Allow DDL only through a trusted migration connection. Generate the exact `CREATE TABLE`
   definition from a reviewed schema model, then inspect `sqlite_schema`, `PRAGMA table_list`, and
   `PRAGMA table_xinfo` in CI. The authorizer's `SQLITE_CREATE_TABLE` event identifies the table but
   does not receive the full column definition, so it can block arbitrary DDL but cannot by itself
   prove that an allowed table has the right ID contract.
4. Enable `PRAGMA foreign_keys=ON` on every writable connection. Run `PRAGMA integrity_check` and
   `PRAGMA foreign_key_check` after migrations. Consider `SQLITE_DBCONFIG_DEFENSIVE` and
   `trusted_schema=OFF` as hardening, while recognizing that neither one is a global ID-policy
   feature. SQLite's own [security guidance](https://www.sqlite.org/security.html) recommends an
   authorizer when an application does not need schema changes.

If an existing SQLite table must gain a different ID type, primary key, or check, the usual safe
operation is a table rebuild: create the new constrained table, copy/validate data, drop the old
table, and rename the new one inside the documented migration procedure. SQLite's limited
`ALTER TABLE` does not generally add/change these contracts in place. See [Making other kinds of
table schema changes](https://www.sqlite.org/lang_altertable.html#making_other_kinds_of_table_schema_changes).

## 5. Decision summary

| Requirement | SurrealDB 3.2+ | SQLite 3.37+ |
|---|---|---|
| Integer key | `DEFINE FIELD id ... TYPE int` | `INT PRIMARY KEY ... STRICT` |
| Reject a text value such as `"123"` for an integer key | Native typed key rejects a string key | `ANY` + `CHECK(typeof(id) = 'integer')` |
| UUID value | Native `TYPE uuid` | `TEXT` + canonical-format `CHECK`, or 16-byte `BLOB` |
| ULID string | `TYPE string DEFAULT rand::ulid() ASSERT id.id().is_ulid()` | `TEXT` + a lexical `CHECK` or trusted validator function |
| Composite structured key | Typed array/object key contract | Composite `PRIMARY KEY`, preferably keeping components separate |
| Prevent ordinary malformed writes | Native write-time `TYPE`/`ASSERT` | `STRICT` + `PRIMARY KEY` + `CHECK` |
| Prevent implicit creation of an unknown table | `DEFINE DATABASE ... STRICT` | Not applicable; `INSERT` never creates a missing table |
| Prevent deliberate arbitrary schema changes | Reserve `OWNER`/`EDITOR` for migrations | Restrict file/connection access and deny DDL with an authorizer |
| Enforce one ID policy on every future table intrinsically | No global meta-schema rule | No global meta-schema rule |
