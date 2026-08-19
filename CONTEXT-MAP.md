# Context Map

## Contexts

- [Aureline Language](./core/ast/CONTEXT.md) — the user-written schema-and-query language and its embedded SurrealQL
- [Static Semantics](./core/checker/CONTEXT.md) — the facts Aureline proves about a program before generation
- [Migration Planning](./core/migration/CONTEXT.md) — versioned schema history and the classified changes rendered as SurrealQL migrations
- [Generated Bindings](./sdks/CONTEXT.md) — target-language functions produced from checked queries

## Relationships

- **Aureline Language → Static Semantics**: declarations and query bodies are resolved and checked against one program model.
- **Static Semantics → Migration Planning**: a Checked Program is lowered into the migration-specific schema facts that can be compared with prior generated state.
- **Static Semantics → Generated Bindings**: only a fully checked query contract may become a generated function.
- **Migration Planning → Aureline Language**: generated migration operations preserve the exact database identities declared by the source program.
- **Generated Bindings → Aureline Language**: generated functions preserve the query text and parameter names declared by the source program.
