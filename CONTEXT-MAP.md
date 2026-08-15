# Context Map

## Contexts

- [Aureline Language](./core/ast/CONTEXT.md) — the user-written schema-and-query language and its embedded SurrealQL
- [Static Semantics](./core/checker/CONTEXT.md) — the facts Aureline proves about a program before generation
- [Generated Bindings](./sdks/CONTEXT.md) — target-language functions produced from checked queries

## Relationships

- **Aureline Language → Static Semantics**: declarations and query bodies are resolved and checked against one program model.
- **Static Semantics → Generated Bindings**: only a fully checked query contract may become a generated function.
- **Generated Bindings → Aureline Language**: generated functions preserve the query text and parameter names declared by the source program.
