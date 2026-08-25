# Parser diagnostics: ownership and timing

**Research date:** 2026-08-22
**Primary sources:** repository domain/ADR documents and GitHub issues queried with `gh`

## Conclusion

Do **not** extract parser problem recognition or recovery into a new reporting subsystem now, and do **not** implement the shared Diagnostic API early. The current plan already draws the useful seam:

```text
parser syntax recognition/recovery -> typed SyntaxProblem
semantic checks                    -> typed Finding
                                      |
                                      v
                         pure renderers in #75
                                      |
                                      v
                 consumer-neutral Diagnostic -> CLI/browser/LSP adapters
```

The parser should continue to own facts that require grammar context, such as a missing union member or postfix `[]`. The later Diagnostic layer should own stable codes, severity, wording, labels, and help. Moving the first category out would either duplicate grammar knowledge or require a richer parser output such as a CST/error-event stream, neither of which is in the current plan.

A small parser-local cleanup is reasonable now when it removes duplicated payloads derivable from spans, consolidates conversion code, or improves parse/recovery structure independently of message wording. Removing the typed problem catalog wholesale would undo completed contracts just before the ticket designed to consume them.

## Settled architecture

- The closed [Diagnostic model #29](https://github.com/pixelscortex/aureline-orm/issues/29) and [diagnostics spec #48](https://github.com/pixelscortex/aureline-orm/issues/48) settle a two-stage model: phases produce typed local problems; pure renderers later produce one consumer-neutral `Diagnostic`. Checks and parsers do not construct terminal, browser, or LSP presentation.
- [Parser spans #56](https://github.com/pixelscortex/aureline-orm/issues/56) intentionally stopped at typed `SyntaxProblem` values with precise source spans. Its acceptance criteria explicitly excluded `Diagnostic`, public codes, and renderers.
- The parser spec [#49](https://github.com/pixelscortex/aureline-orm/issues/49) requires typed syntax problems, while meaning-free type syntax leaves type validity to static semantics. The completed parser tickets [#63](https://github.com/pixelscortex/aureline-orm/issues/63), [#64](https://github.com/pixelscortex/aureline-orm/issues/64), [#65](https://github.com/pixelscortex/aureline-orm/issues/65), [#66](https://github.com/pixelscortex/aureline-orm/issues/66), and [#67](https://github.com/pixelscortex/aureline-orm/issues/67) deliberately added durable typed facts for identifier boundaries, comments, applications, unions, and tuples while deferring wording/codes to #75.
- The local domain language agrees with that split: [Static Semantics](../../core/checker/CONTEXT.md) defines a **Finding** as a typed semantic problem and a **Diagnostic** as the later consumer-neutral report. [ADR 0002](../adr/0002-four-rules-against-compiler-sprawl.md) requires explicit problems instead of silent fallbacks, but also forbids infrastructure before a feature needs it.

## Current footprint

The present parser catalog has 12 public `SyntaxProblem` variants and seven `IdentifierProblem` subcategories in `core/parser/src/problem.rs`. The grammar has 11 internal `GrammarProblem` variants and one conversion boundary in `core/parser/src/grammar/problem.rs`.

References to `SyntaxProblem`, `IdentifierProblem`, `GrammarProblem`, or the valid/recovered type-expression carrier occur in 17 production files under `core/parser/src`; `GrammarProblem` itself occurs in ten grammar files. This is a broad footprint, but it is not all “report rendering”:

- **Reporting-shaped and movable later:** stable identity, severity, prose, primary/secondary labels, help, serialization, and consumer presentation. None of that is implemented in the parser today.
- **Parser-owned:** recognizing malformed token shapes, choosing the offending span, consuming enough malformed syntax to avoid misleading follow-on failures, and preserving one typed failure through recursive applications/unions/tuples.
- **Potential cleanup:** duplicate data already recoverable from a span/source, special distinctions with no expected downstream action, and recovery alternatives that reparse shared prefixes. These can be simplified inside the parser without inventing the Diagnostic API.

The distinction matters when estimating extraction cost. Moving the two problem enums/conversion table is small; moving all apparent problem-related code is a parser redesign because recovery is interleaved with recursive grammar control flow.

## Dependency and timing map

| Work | Current state and dependency | Relevance |
|---|---|---|
| [#55 Findings/recovery](https://github.com/pixelscortex/aureline-orm/issues/55) | Open and currently unblocked | Adds deterministic semantic collection and `Reported` proof, explicitly without the Diagnostic model. |
| [#68 semantic index](https://github.com/pixelscortex/aureline-orm/issues/68) | Open; natively blocked by #55 (and a closed parser ticket) | First real semantic Findings; native blocker for #75. |
| [#69 scalar resolution](https://github.com/pixelscortex/aureline-orm/issues/69) | Blocked by #68 | Adds unknown/unsupported-type Findings and the Checked Program. |
| [#70–#73 semantic type families](https://github.com/pixelscortex/aureline-orm/issues/70) | Each follows #69 | Adds collection, record-link, union/presence, and tuple Findings. |
| [#74 record-key contract](https://github.com/pixelscortex/aureline-orm/issues/74) | Blocked by #70–#73 | Completes the table-slice semantic problem set. |
| [#75 Diagnostic API](https://github.com/pixelscortex/aureline-orm/issues/75) | Open; native graph says blocked only by #68 | Its text says it lands last in core and consumes parser problems plus Findings from #68–#74. Treat that prose as the intended timing. |
| [#57 terminal adapter](https://github.com/pixelscortex/aureline-orm/issues/57) | Open; native graph blocks it on #75 and CLI spec #53 | Presentation belongs after the shared model. Its body still lists only old blocker #56, so the native graph is more current here. |
| [#58 browser envelope](https://github.com/pixelscortex/aureline-orm/issues/58) | Closed/parked | Explicitly deferred until a playground is a real consumer; #75 need only keep the model serializable. |

There is one tracker inconsistency worth resolving before scheduling #75: its body says the renderer should be designed against Findings from all of #68–#74, but its native dependency graph blocks it only on #68. Starting immediately after #68 would satisfy the graph but contradict the ticket's stated rationale and ADR 0002.

## Recommendation

1. Proceed with #55 and the semantic sequence. Treat #75 as following the actual semantic catalog, preferably #68–#74, rather than as a prerequisite for it.
2. Keep parser outputs typed and phase-local. Do not replace required variants from #63–#67 with generic parser-library errors merely to reduce file count; #75 needs concrete conditions to decide which distinctions deserve stable public identities.
3. Allow parser-local simplification now under a strict test: it must improve parsing/recovery or remove redundant data without introducing codes, messages, severity, label models, registries, or consumer concerns.
4. At #75, centralize only the pure mapping from phase-local problems to `Diagnostic`. After those mappings exist, review which parser distinctions map identically and have no tooling value; that is the evidence-based point to merge variants.
5. Align #75's native blockers with its written “lands last” plan before implementation, so the frontier cannot schedule it prematurely.

This preserves the clean seam already chosen while avoiding both extremes: deleting useful parser facts now, or building a speculative reporting framework before the semantic engine supplies its real cases.
