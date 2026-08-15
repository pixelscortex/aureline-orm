# Aureline is a static compiler, not a runtime checker

Aureline parses and statically checks user-written schema and query declarations, then generates SurrealQL and typed host-language functions. It does not validate values at runtime, execute queries itself, or own a database client; generated functions receive a host-provided Database Context. This keeps the semantic contract deterministic and target-neutral while leaving execution policy to each SDK and application.
