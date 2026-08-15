# Static Semantics

Static Semantics is the context in which Aureline establishes identities, types, scopes, and query results without connecting to a running database.

## Analysis

**Source Fact**:
A lossless indexed observation about a declaration, including invalid or duplicate declarations that later checks may diagnose.
_Avoid_: Validated symbol, resolved node

**Resolution**:
The act of connecting a source name or path to a unique declared identity.
_Avoid_: Lookup, validation

**Static Environment**:
The names and types available within one Embedded SurrealQL context.
_Avoid_: Runtime context, values

**Checked Program**:
An Aureline Program for which all generation-blocking static obligations have been proven.
_Avoid_: Valid AST, compiled string

## Types and results

**Any**:
An explicitly dynamic SurrealDB type that accepts values by language contract.
_Avoid_: Unknown, fallback

**Unknown**:
A recovery state meaning analysis has not established a useful type.
_Avoid_: Any, inferred any

**Error Type**:
A recovery state meaning a diagnostic already accounts for the failed type derivation.
_Avoid_: Unknown, invalid value

**Row Shape**:
The named fields and value types produced for one selected result value.
_Avoid_: Table type, response object

**Cardinality**:
The statically inferred count form of a query result, such as one value or a collection.
_Avoid_: Array type, nullability

**Nullability**:
Whether a query result may contain or be the database's null-like absence value.
_Avoid_: Cardinality, optional parameter

**Query Result**:
The combined static contract of a query's value type or Row Shape, Cardinality, and Nullability.
_Avoid_: Return type when the dimensions have not been distinguished

## Reporting

**Finding**:
A semantic fact produced by a check that may later be rendered for a user.
_Avoid_: Error message, diagnostic text

**Diagnostic**:
A user-facing rendering of a Finding with stable identity, severity, source labels, and guidance.
_Avoid_: Finding, exception

**Opaque Syntax**:
Source preserved for tooling because Aureline cannot yet assign it supported static meaning.
_Avoid_: Checked syntax, Any
