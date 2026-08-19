# Aureline Language

Aureline is a statically checked schema-and-query language whose programs describe SurrealDB data and operations without executing them.

## Language

**Aureline Program**:
A source document containing declarations that together define a schema and its statically checked operations.
_Avoid_: ORM configuration, runtime schema

**Table**:
A named record collection with a declared schema mode and fields. Its declared name is its exact, case-sensitive database identity; Aureline prescribes no casing style.
_Avoid_: Model, entity

**Relation Table**:
A table whose records connect declared source and destination record kinds.
_Avoid_: Join table, relationship model

**Field**:
A named value declared on a table, including its source type and field contract. Its declared name is case-sensitive, and Aureline prescribes no casing style.
_Avoid_: Property, column

**Function**:
A named, typed callable declared as part of the database schema.
_Avoid_: Helper, method

**Query**:
A named, typed operation whose body describes SurrealQL and whose parameters and result form a static contract.
_Avoid_: Request, repository method

**Run Block**:
The source region inside a query or function that contains its embedded SurrealQL body.
_Avoid_: Raw SQL, callback

**Parameter**:
A named typed input made available to a query's Run Block and later supplied as a database binding.
_Avoid_: Argument when referring to the declaration

**Declared Result**:
The source-level result type promised by a query or function and checked against the inferred result.
_Avoid_: Return annotation

**Embedded SurrealQL**:
SurrealQL source hosted in a context-specific Aureline slot such as a Run Block, field assertion, permission, or event condition.
_Avoid_: Raw string, SQL blob
