# Static Semantics

Static Semantics is the context in which Aureline establishes identities, types, scopes, and query results without connecting to a running database.

## Analysis

**Source Fact**:
A lossless indexed observation about a declaration, including invalid or duplicate declarations that later checks may diagnose.
_Avoid_: Validated symbol, resolved node

**Resolution**:
The act of connecting a source name or path to a unique declared identity.
_Avoid_: Lookup, validation

**Type Resolution Outcome**:
The analysis result of resolving a declared source type: a Semantic Type, an unknown state, or an invalid state proving a Finding was reported.
_Avoid_: Semantic Type when resolution did not succeed

**Unknown Type Resolution**:
A recovery outcome meaning analysis has not established a useful type and no diagnostic is guaranteed.
_Avoid_: Any, invalid type

**Invalid Type Resolution**:
A recovery outcome carrying proof that a Finding already accounts for failed type resolution. Dependent analysis propagates it without cascading, while independent checks still report their own Findings.
_Avoid_: Error Type, failed generation

**Static Environment**:
The names and types available within one Embedded SurrealQL context.
_Avoid_: Runtime context, values

**Checked Program**:
Aureline's complete, target-neutral semantic result, produced only when every generation-blocking static obligation has been proven. It contains the proven facts every downstream consumer needs, without consumer-specific representations or unknown and invalid analysis outcomes.
_Avoid_: Valid AST, compiled string

## Types and results

**Semantic Type**:
A resolved, target-neutral SurrealDB value contract corresponding to a source-declared type. Field presence, source syntax, and target-language representation are separate concerns.
_Avoid_: Source type, TypeScript type, field shape

**Any**:
An explicitly dynamic SurrealDB type that accepts values by language contract.
_Avoid_: Unknown, fallback

**Row Shape**:
The named fields and value types produced for one selected result value.
_Avoid_: Table type, response object

**Cardinality**:
The statically inferred count form of a query result, such as one value or a collection.
_Avoid_: Array type, nullability

**Field Presence**:
Whether a declared field may be absent from a record. It is permitted by SurrealDB `NONE`, including through `option<T>`, and is distinct from storing `NULL`.
_Avoid_: Nullability, nullable field

**Nullability**:
Whether a value may be SurrealDB's stored `NULL` value.
_Avoid_: Field presence, `NONE`, optional parameter

**Query Result**:
The combined static contract of a query's value type or Row Shape, Cardinality, and Nullability.
_Avoid_: Return type when the dimensions have not been distinguished

## Reporting

**Finding**:
A typed semantic problem produced by a check, before stable identity, wording, severity, and presentation are assigned.
_Avoid_: Error message, diagnostic text

**Diagnostic**:
A consumer-neutral report rendered from a phase-local typed problem such as a Finding. It has an immutable public code, an error-or-warning severity, source labels, and guidance; errors block generation while warnings do not.
_Avoid_: Finding, exception

**Opaque Syntax**:
Source preserved for tooling because Aureline cannot yet assign it supported static meaning.
_Avoid_: Checked syntax, Any
