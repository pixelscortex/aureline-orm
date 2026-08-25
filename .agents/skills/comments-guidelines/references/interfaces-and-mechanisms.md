# Interface and Mechanism Contrasts

## Contents

- [Document a supported interface for independent use](#document-a-supported-interface-for-independent-use)
- [Lead a difficult mechanism with its stages](#lead-a-difficult-mechanism-with-its-stages)
- [Trace a recovery parser across its seams](#trace-a-recovery-parser-across-its-seams)
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

## Trace a recovery parser across its seams

**Context:** An Aureline field parser accepts `name string`, recognizes malformed
declared names, carries recovered recursive-type problems, and delays arena
allocation until its table has no problem. Every alternative must reach the same
physical field boundary, and alternative order affects which diagnostic wins.

**Don't:** Describe only the valid grammar or narrate each combinator. A reader
still has to reverse-engineer why there are several alternatives, what their
ordering controls, and when the AST changes.

```rust
//! Parses `<name> <type>` fields.

// Parse a name.
let field = ident().then(type_expression());
// Handle a split name.
let split_name = ident().then(ident()).then(type_expression());
choice((field, split_name))
```

**Do:** Give the owning module a compact trace across the lexer, grammar, and AST
seams, then comment only locally significant precedence or recovery choices.

```rust
//! Parses one physical field into a staged valid field or a recoverable problem.
//!
//! Every alternative must reach newline or `}`. The valid branch may still
//! carry a recovered type problem; name-recovery branches classify malformed
//! token shapes. Table parsing selects the earliest problem and allocates the
//! staged fields only when none exists, so rejected tables cannot enter the AST.
//!
//! Representative flow:
//! `owner record<User | Bot>` -> staged field named `owner` with an application
//! type -> `FieldDecl` after its table commits.
//! `first name string` -> split-name recovery -> identifier-whitespace problem
//! -> no `FieldDecl` allocation.

// Keep the marked-shape recovery before the general punctuation recovery:
// `array<string> bool` must report `<`, not a later token.
choice((valid_field, split_name, marked_name, punctuated_name))
```

**Why:** A maintainer can follow concrete user input through intermediate and
committed representations, see which ordering is semantic, and preserve the
no-partial-output invariant without reading every combinator and caller first.

**Exception:** A linear parser whose names and return type already expose its
only path needs no stage trace merely because parsing is important.

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
