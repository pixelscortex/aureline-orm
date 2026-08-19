# Generated Bindings

Generated Bindings are target-language types and functions produced from a Checked Program. They describe complete table documents and expose checked Aureline queries through a host application's database context.

## Language

**Generated Query Artifact**:
A target-neutral checked description containing query text, bindings, Query Result, and source identity for one Query.
_Avoid_: SDK AST, template context

**Generated Function**:
A host-language function rendered from one Generated Query Artifact.
_Avoid_: Runtime query builder, handwritten wrapper

**Binding**:
The runtime association between a declared Parameter and the value passed to SurrealDB.
_Avoid_: String interpolation, argument

**Target Renderer**:
A generator that renders one Checked Program as the complete Generated Bindings for a host language without performing semantic checking. One renderer owns that language's document types and generated functions.
_Avoid_: Compiler backend when it implies target-specific checking

**Database Context**:
The host-provided capability a Generated Function uses to submit query text and bindings.
_Avoid_: Global client, runtime checker

**Generated Document Type**:
The target-language type of a complete record selected from one declared Table, including its record identity and declared fields.
_Avoid_: Row type, declared field shape, writable input

**Table Type Registry**:
A target-language type-level map from each exact Table identity to its Generated Document Type.
_Avoid_: Runtime registry, table descriptor

**Reference Expansion**:
The Generated Document Type shape in which explicitly named record-reference fields contain linked Generated Document Types instead of only record identities. It describes already-fetched data and never requests it.
_Avoid_: Projection, eager loading, fetch instruction

**Expansion Tree**:
A finite nested description of the record-reference fields represented through Reference Expansion. Ordinary fields never belong to it, and omitted references remain record identities.
_Avoid_: With expression, dotted path, field selection
