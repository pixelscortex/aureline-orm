---
name: comments-guidelines
description: "Curate code comments as local design records. Use when creating or changing code and documentation comments, file or module documentation, usage examples, TODOs, tool suppressions, compatibility workarounds, or generated-code notices; when reviewing those records; or when a semantic code change may invalidate nearby comments."
---

# Comments Guidelines

Treat a comment as a curated local design record: preserve current meaning, decisions, and reasoning beside the code they explain. Add comments selectively; clarity without a comment is a successful outcome.

## Decide in context

1. Inspect the declaration in its real surroundings. Read its location, file or module documentation, names, types, enclosing logic, and nearby comments before judging what remains unexplained. Inspect callers, history, issues, or domain records when needed to recover a fact, then communicate the relevant local consequence or point to its authoritative source. Finish when the meaning already supplied nearby and the meaning still hidden are both explicit.

2. Improve the code first where practical. Let names, types, decomposition, or structure carry meaning they express naturally. Treat code length, number of concepts or stages, control flow, state transitions, distance between interacting parts, hidden constraints or assumptions, and cost of misunderstanding as signals to inspect more closely, never as verdicts. One line can hide a costly decision, while longer code can already be clear. Finish when code carries every candidate meaning it can express clearly and only residual comment candidates remain.

3. Apply one admission test. Add or retain a comment only when, without it, a reasonable beginner or agent could misunderstand the design, misuse the interface, repeat expensive investigation, or make a plausible harmful change. Otherwise leave clear code uncommented or remove narration that merely translates it. Finish when every candidate has passed the test or been omitted.

4. Choose the applicable semantic roles for the remaining meaning, then apply the matching flat rules below. Finish when every admitted record has a stable owner and every relevant rule has been applied.

When a decision remains ambiguous, load only the matching contrast reference before editing:

- For admission, code length, module orientation, ownership, or ADR placement, read [Context and placement](references/context-and-placement.md).
- For a supported interface, difficult mechanism, or language-native documentation, read [Interfaces and mechanisms](references/interfaces-and-mechanisms.md).
- For concrete rationale, high-risk constructs, future guidance, or TODOs, read [Decisions and risks](references/decisions-and-risks.md).
- For stale comments, change scope, or generated output, read [Maintenance and generation](references/maintenance-and-generation.md).

Each reference compares real surrounding contexts and pairs a weak treatment with the information-preserving alternative.

## Apply the rules

- **File or module orientation:** Explain why a non-obvious file or module exists, the role it plays, its shared mental model, or its relationship to the larger program. Use the language's actual file or module documentation form. Let an already-clear file remain without a banner.

- **Supported interfaces:** Make an intentionally supported interface understandable without its implementation. State purpose; when to use or avoid it; constraints on correct use; meaningful errors, ordering, configuration, or performance behavior; and a contextual input/output example when use remains difficult to predict. Keep implementation details inside the module unless callers must act on them.

- **Local decisions:** Record the reason that produced the current choice and any consequence that constrains later changes. Include a plausible rejected alternative only when the record can prevent its accidental reintroduction.

- **Difficult mechanisms:** Start genuinely complicated logic with an enclosing mental model or stage overview. Add local comments only where a step's significance remains hidden after that overview.

- **Critical parser and AST mechanisms:** Orient the owning module when correctness depends on recovery or precedence, lexer-only layout or span channels, deferred AST construction, source-provenance translation, or arena ownership. State the representations crossing each seam, the stage or ordering whose change would alter results, and the commit condition or invariant that prevents partial output. Include a compact worked flow from representative user input or grammar pattern, through important intermediate values, to the resulting AST or arena relationships; include an invalid flow when recovery is part of the mechanism. Use one general example rather than cataloguing every syntax form. This is an orientation requirement, not a comment-count rule; leave straightforward combinators, data declarations, and wiring to clear code.

- **Placement and authority:** State each meaning once at the narrowest stable scope that covers all affected code. Keep evolving, cross-cutting rationale in its ADR or domain document; summarize only the consequence needed locally. Load `codebase-design` whenever the comment decision requires reasoning about a module, interface, seam, or adapter; use its vocabulary instead of redefining it here. Treat intent, usage, decision, mechanism, and example as prompts for content, not mandatory headings or prefixes. Prefer idiomatic structure over universal tags or an indexing scheme.

- **Language-native documentation:** Use TSDoc where applicable in TypeScript and Rust documentation conventions where applicable in Rust. Match comment ownership to syntax: item documentation belongs to the following item, while file or module documentation belongs to its containing scope. Make usage examples executable through the repository's normal documentation or test mechanism whenever practical; use a plain example when execution would add disproportionate ceremony.

- **High-risk constructs:** Supply a concrete rationale for tool suppressions, unsafe operations, compatibility workarounds, unusual constants, and counterintuitive optimizations when surrounding context does not already supply it. Preserve tool- or framework-required comments. Give every temporary construct a verifiable removal condition.

- **Future guidance and TODOs:** Keep future-facing guidance when it extends a current decision and states why later implementations inherit it. Keep speculative feature promises in planning records instead. Write a TODO only when it names concrete remaining work and the present blocker, trigger, or removal condition; link an existing issue when useful without creating one solely for the comment.

- **Maintenance and generated code:** Update or remove a local design record when the decision it describes changes. Review comments intersecting the semantic change and keep unrelated cleanup out of scope. Put manually maintained reasoning for generated output in the authoritative generator or source while preserving required generated notices.

## Complete the change

Check every region created or semantically changed, plus every nearby comment whose meaning intersects the change; use no target count or percentage. Finish only when each important context gap is resolved by the code or recorded at the appropriate scope, each affected comment is accurate, and every practical executable example still passes. Leave unrelated comment cleanup to a dedicated task.
