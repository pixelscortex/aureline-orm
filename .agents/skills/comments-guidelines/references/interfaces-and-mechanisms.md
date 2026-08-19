# Interface and Mechanism Contrasts

## Contents

- [Document a supported interface for independent use](#document-a-supported-interface-for-independent-use)
- [Lead a difficult mechanism with its stages](#lead-a-difficult-mechanism-with-its-stages)
- [Use native documentation and executable examples](#use-native-documentation-and-executable-examples)

## Document a supported interface for independent use

**Context:** A hypothetical TypeScript package exports `retry` as an intentionally supported interface. It accepts only operations that are safe to repeat, stops before the next attempt when its signal aborts, and rejects with the final operation error after exhausting its attempts.

**Don't:** Restate the signature or reveal irrelevant retry-loop details.

```ts
/** Retries an operation. */
export function retry<T>(
  operation: () => Promise<T>,
  options: RetryOptions,
): Promise<T> { /* ... */ }
```

**Do:** Document the choices and outcomes a caller needs, using the repository's TSDoc form.

```ts
/**
 * Retries an idempotent asynchronous operation.
 *
 * @remarks
 * Use this for operations that are safe to repeat. Aborting `options.signal`
 * stops before the next attempt. Exhaustion rejects with the final operation
 * error. Avoid this interface for one-shot writes without an idempotency key.
 *
 * @example Retry an idempotent metadata read
 * ```ts
 * const metadata = await retry(() => loadMetadata(), {
 *   attempts: 3,
 *   signal,
 * });
 * ```
 */
export function retry<T>(
  operation: () => Promise<T>,
  options: RetryOptions,
): Promise<T> { /* ... */ }
```

**Why:** Callers can choose the correct context and handle every meaningful outcome without reading the implementation.

**Exception:** Keep documentation short when the interface's purpose and complete usage contract are already obvious; omit sections that add no caller-relevant information.

## Lead a difficult mechanism with its stages

**Context:** A hypothetical deployment tool classifies infrastructure changes, orders dependent operations, and renders a provider plan. The stages are individually named, but their ordering is the mechanism's key constraint.

**Don't:** Narrate each statement and leave the reader to reconstruct the pipeline.

```rust
// Classify changes.
let changes = classify(previous, current)?;
// Sort changes.
let ordered = order(changes)?;
// Render changes.
render(ordered)
```

**Do:** Give the enclosing scope a stage overview, then mark only a locally surprising step.

```rust
// Process in three stages: classify semantic changes, order operations by
// dependency, then render the provider plan. Rendering stays last so provider
// details cannot influence destructive-change classification.
let changes = classify(previous, current)?;
let ordered = order(changes)?;

// Preserve source order among independent operations to keep output stable.
let ordered = stabilize_independent_operations(ordered);
render(ordered)
```

**Why:** The overview supplies a mental model; the local note identifies significance that names and control flow still cannot express.

**Exception:** Use local comments without an overview when only one isolated step is difficult and the enclosing mechanism is already clear.

## Use native documentation and executable examples

**Context:** Aureline's Rust parser exposes an entrypoint that returns an arena-backed AST. Users need to see a complete table declaration and how to inspect the parsed root.

**Don't:** Use a foreign tag vocabulary or a non-running sketch when a doctest is practical.

```rust
/// @example Parse a table.
/// parse("table User schemafull {}")
pub fn parse(source: &str) -> Result<Ast, Vec<EmptyErr>> { /* ... */ }
```

**Do:** Use rustdoc headings and an assertion that the normal documentation test can execute.

```rust
/// Parses one Aureline source file into an arena-backed AST.
///
/// # Examples
///
/// ```
/// use aureline_parser::parse;
///
/// let ast = parse("table User schemafull {}").unwrap();
/// assert_eq!(ast.root().items().len(), 1);
/// ```
pub fn parse(source: &str) -> Result<Ast, Vec<EmptyErr>> { /* ... */ }
```

**Why:** Native structure integrates with the language's documentation tooling, and execution detects stale examples.

**Exception:** Use a plain comment example for small, stable behavior when wiring an executable example would cost more ceremony than confidence; still make inputs and outputs concrete.
