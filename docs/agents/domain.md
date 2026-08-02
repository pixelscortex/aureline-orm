# Domain Docs

How engineering skills should consume this repository’s domain documentation.

## Before exploring, read these

- `CONTEXT-MAP.md` at the repository root. It points to the `CONTEXT.md` files relevant to each context.
- `docs/adr/` for system-wide decisions.
- Context-local `docs/adr/` directories for decisions scoped to a component.

If these files do not exist, proceed silently. The domain-modeling workflow creates them lazily when terminology or architectural decisions are resolved.

## File structure

This is a multi-context repository:

```
/
├── CONTEXT-MAP.md
├── docs/adr/                    ← system-wide decisions
├── core/<context>/
│   ├── CONTEXT.md
│   └── docs/adr/                ← core context decisions
├── sdks/<context>/
│   ├── CONTEXT.md
│   └── docs/adr/                ← SDK decisions
└── site/
    ├── CONTEXT.md
    └── docs/adr/                ← site decisions
```

Only create context documents when a context has meaningful terminology or decisions to record.

## Use the glossary’s vocabulary

When output names a domain concept—in an issue title, proposal, hypothesis, or test—use the term defined in the relevant `CONTEXT.md`.

If a needed concept is absent, reconsider whether the term fits the project or note the gap for domain modeling.

## Flag ADR conflicts

If proposed work contradicts an existing ADR, surface the conflict explicitly instead of silently overriding it.
