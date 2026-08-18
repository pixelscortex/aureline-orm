# Context and Placement Contrasts

## Contents

- [Keep shared reasoning at its owner](#keep-shared-reasoning-at-its-owner)
- [Leave clear internal code alone](#leave-clear-internal-code-alone)
- [Orient only the files that need a mental model](#orient-only-the-files-that-need-a-mental-model)
- [Judge short and long code by hidden meaning](#judge-short-and-long-code-by-hidden-meaning)
- [Keep cross-cutting rationale authoritative](#keep-cross-cutting-rationale-authoritative)

## Keep shared reasoning at its owner

**Context:** A Rust lease module renews ten seconds early because the backing store may expire a lease while a delayed renewal is in flight. Several functions inherit that decision.

**Don't:** Repeat the module's reasoning on every function.

```rust
//! Renews leases ten seconds before expiry so scheduler delay cannot let the
//! backing store expire a live worker's lease.

/// Renews ten seconds early so scheduler delay cannot expire the lease.
fn renewal_at(expires_at: Instant) -> Instant {
    expires_at - Duration::from_secs(10)
}
```

**Do:** Keep the shared mental model in the module documentation and let the clear helper participate in it without repetition.

```rust
//! Renews leases ten seconds before expiry so scheduler delay cannot let the
//! backing store expire a live worker's lease.

fn renewal_at(expires_at: Instant) -> Instant {
    expires_at - Duration::from_secs(10)
}
```

**Why:** One record at the owning scope stays consistent and gives readers the reason before they inspect any participating function.

**Exception:** Put an additional note on a supported function when callers encounter it independently and must act on a constraint not communicated by the module documentation.

Now place the same implementation in a generic scheduling module with no lease context.

**Don't:** Leave the unexplained offset looking arbitrary or narrate the subtraction.

```rust
fn renewal_at(expires_at: Instant) -> Instant {
    // Subtract ten seconds.
    expires_at - Duration::from_secs(10)
}
```

**Do:** Record the hidden decision and consequence beside the code it constrains.

```rust
fn renewal_at(expires_at: Instant) -> Instant {
    // Leave enough time for scheduler delay; renewing at expiry can let the
    // backing store remove a lease that still protects a live worker.
    expires_at - Duration::from_secs(10)
}
```

**Why:** Identical code needs different treatment when its surroundings carry different meaning.

## Leave clear internal code alone

**Context:** A private helper normalizes a key exactly as its name, types, and expression state.

**Don't:** Translate the expression into English.

```ts
// Trim whitespace and convert the key to lowercase.
function normalizeKey(key: string): string {
  return key.trim().toLowerCase();
}
```

**Do:** Let the code explain itself.

```ts
function normalizeKey(key: string): string {
  return key.trim().toLowerCase();
}
```

**Why:** Narration competes with records that carry information unavailable from the code.

**Exception:** Explain a surprising normalization rule when a compatibility contract or external identity scheme makes the apparently equivalent implementation unsafe.

## Orient only the files that need a mental model

**Context:** A parser file coordinates recovery after an invalid declaration. Its helper names describe their local actions, but none explains why recovery stops at the next top-level keyword.

**Don't:** Add a generic banner or repeat the same recovery rationale on every helper.

```rust
// Parser helpers.
fn skip_invalid_tokens(cursor: &mut Cursor) { /* ... */ }
fn find_next_declaration(cursor: &mut Cursor) { /* ... */ }
fn resume_parsing(cursor: &mut Cursor) { /* ... */ }
```

**Do:** State the shared purpose and consequence once in module documentation.

```rust
//! Recovers at top-level declaration keywords. Aureline declarations cannot
//! nest, so this boundary preserves later diagnostics without treating tokens
//! inside the invalid declaration as new syntax.

fn skip_invalid_tokens(cursor: &mut Cursor) { /* ... */ }
fn find_next_declaration(cursor: &mut Cursor) { /* ... */ }
fn resume_parsing(cursor: &mut Cursor) { /* ... */ }
```

**Why:** A reader receives the whole-file mental model before interpreting the helpers, while the local code remains uncluttered.

**Exception:** Let a file whose path, names, types, and enclosing context already make its role apparent remain without module documentation.

## Judge short and long code by hidden meaning

**Context:** A sharded poller uses a prime interval so fleets started together do not remain aligned with a 256-bucket scheduler.

**Don't:** Treat a one-line declaration as self-explanatory because it is short.

```ts
const POLL_INTERVAL_MS = 257;
```

**Do:** Preserve the decision that makes the unusual constant safe to change.

```ts
// A prime interval prevents synchronized pollers from repeatedly aligning with
// the scheduler's 256 buckets.
const POLL_INTERVAL_MS = 257;
```

**Why:** The expression is simple while the choice and consequence are not.

Now consider a longer transition whose enum variants and function names already expose the behavior.

**Don't:** Add a comment merely because the function spans several branches.

```rust
// Handle each job state.
match job.state {
    JobState::Queued => start(job),
    JobState::Running => observe(job),
    JobState::Finished => archive(job),
}
```

**Do:** Let the explicit states and actions carry the meaning.

```rust
match job.state {
    JobState::Queued => start(job),
    JobState::Running => observe(job),
    JobState::Finished => archive(job),
}
```

**Why:** Line count is only a reason to inspect; it is not evidence that meaning is missing.

**Exception:** Comment a long block when its names, types, and structure still conceal an ordering rule, constraint, assumption, or consequence.

## Keep cross-cutting rationale authoritative

**Context:** Aureline's system-wide architecture requires syntax to be walked once into a semantic index. A checker needs a local reminder that it must read the index instead of traversing syntax again.

**Don't:** Copy the architectural history and every affected rule into an implementation comment.

```rust
// Earlier Aureline versions became hard to change because multiple passes
// walked the syntax tree. The compiler now uses one walk to build an index,
// keeps syntax and semantic types separate, forbids silent fallbacks, and only
// builds infrastructure when a feature needs it.
fn check_table(index: &SemanticIndex, table: TableId) { /* ... */ }
```

**Do:** State the consequence this location must obey and point to the record that owns the cross-cutting rationale.

```rust
// Read the semantic index here; re-walking syntax would split name resolution
// across passes. See ADR 0002.
fn check_table(index: &SemanticIndex, table: TableId) { /* ... */ }
```

**Why:** The ADR preserves the decision and its evolution once, while the local record explains how that decision constrains this code.

**Exception:** Keep reasoning entirely local when the choice affects only this implementation and no cross-cutting record owns it.
