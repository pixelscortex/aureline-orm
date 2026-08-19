# Decision and Risk Contrasts

## Contents

- [Replace vague rationale with a concrete cause](#replace-vague-rationale-with-a-concrete-cause)
- [Explain suppressions, unsafe code, and temporary workarounds](#explain-suppressions-unsafe-code-and-temporary-workarounds)
- [Separate grounded future guidance from speculation](#separate-grounded-future-guidance-from-speculation)
- [Make TODOs actionable](#make-todos-actionable)

## Replace vague rationale with a concrete cause

**Context:** A SQLite insert batches seven bindings per row under SQLite's 999-variable limit.

**Don't:** Give a reason too vague to guide a future edit.

```ts
// Use 128 for performance.
const INSERT_BATCH_SIZE = 128;
```

**Do:** Name the external limit, the calculation, and the consequence.

```ts
// Each row contributes seven bindings; 128 leaves headroom below SQLite's
// 999-variable limit for the statement's fixed parameters.
const INSERT_BATCH_SIZE = 128;
```

**Why:** A maintainer can recalculate the value when the row shape or database changes instead of preserving or replacing a magic number blindly.

**Exception:** Mention a rejected batch size only when it is a plausible alternative and the reason it failed is not already implied by the stated constraint.

## Explain suppressions, unsafe code, and temporary workarounds

**Context:** A React subscription intentionally depends on connection identity rather than the current callback. Re-subscribing for every render can drop events between unsubscribe and subscribe.

**Don't:** Leave the lint suppression to look like expedience.

```ts
// eslint-disable-next-line react-hooks/exhaustive-deps
useEffect(() => subscribe(connection, handler), [connection]);
```

**Do:** Put the concrete behavioral reason beside the required directive.

```ts
// Keep one subscription for each connection: re-subscribing when `handler`
// changes creates a gap in which events are lost.
// eslint-disable-next-line react-hooks/exhaustive-deps
useEffect(() => subscribe(connection, handler), [connection]);
```

**Why:** A future contributor can distinguish a deliberate exception from a forgotten lint fix.

**Exception:** Let a repository-wide configuration own the rationale when it already communicates the same constraint at the effective scope.

**Context:** A Rust adapter constructs a slice from a driver-owned buffer whose lifetime is guaranteed until the next driver call.

**Don't:** Make the safety claim circular.

```rust
// SAFETY: The pointer is valid.
let bytes = unsafe { slice::from_raw_parts(ptr, len) };
```

**Do:** State the proof obligation and the operation that ends it.

```rust
// SAFETY: The driver owns `ptr..ptr + len` and keeps it initialized and
// immovable until the next driver call; `bytes` is consumed before that call.
let bytes = unsafe { slice::from_raw_parts(ptr, len) };
```

**Why:** The record exposes the assumption that must remain true when surrounding control flow changes.

**Exception:** Preserve a required `SAFETY` form, but let the text prove the actual preconditions instead of treating the heading as the proof.

**Context:** A compatibility branch remains until the minimum supported media driver exposes `decode_frame`.

**Don't:** Label it temporary without a removal trigger.

```ts
// Temporary driver workaround.
return driver.decode(encodeLegacyFrame(frame));
```

**Do:** Name the compatibility boundary that makes deletion verifiable.

```ts
// Remove the legacy encoding path when the minimum supported driver is 3.2;
// earlier versions do not expose `decode_frame`.
return driver.decode(encodeLegacyFrame(frame));
```

**Why:** Temporary code becomes removable based on a fact rather than memory.

**Exception:** Describe the supported compatibility range without a removal condition when the branch is an intentional permanent part of that range.

## Separate grounded future guidance from speculation

**Context:** Every filesystem adapter must preserve object keys exactly because the backing store treats differently cased keys as distinct objects.

**Don't:** Turn the comment into an unrelated roadmap.

```rust
// Add cloud adapters and automatic key casing later.
```

**Do:** Extend the current decision to future implementations and retain its reason.

```rust
// New storage adapters must preserve object keys verbatim; normalizing them can
// make two distinct objects collide.
```

**Why:** The guidance constrains future work using a present invariant, while feature ideas belong in an issue or roadmap.

**Exception:** Keep a future possibility out of implementation comments when no current decision, constraint, or consequence supports it.

## Make TODOs actionable

**Context:** In a hypothetical GitHub-backed schema tool, SQL generation cannot emit `ALTER FIELD` until change classification distinguishes type changes from assertion changes. Assume a numbered issue already tracks that classification work.

**Don't:** Record only dissatisfaction or a vague intention.

```rust
// TODO: Handle this better later.
```

**Do:** Name the remaining work and its present blocker; replace `<number>` with the existing issue's number when one is useful.

```rust
// TODO(#<number>): Emit `ALTER FIELD` after change classification separates
// type changes from assertion-only changes; the current change kind cannot
// select safe syntax.
```

**Why:** The next contributor can tell what completion means and what must change first.

**Exception:** Do not create an issue solely to decorate a small, self-contained TODO.
