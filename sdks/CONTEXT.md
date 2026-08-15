# Generated Bindings

Generated Bindings are target-language functions that expose checked Aureline queries through a host application's database context.

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
A generator that maps Generated Query Artifacts into one host language without performing semantic checking.
_Avoid_: Compiler backend when it implies target-specific checking

**Database Context**:
The host-provided capability a Generated Function uses to submit query text and bindings.
_Avoid_: Global client, runtime checker
