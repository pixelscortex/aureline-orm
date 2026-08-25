# Parser documentation rules

Apply the repository `comments-guidelines` skill whenever parser code or parser comments change. Parser combinators expose implementation shape more readily than source-language meaning, so every changed parser interface must carry the missing orientation locally.

For each parser module or parser-producing function (`fn ... -> impl Parser`):

1. Keep the module's `parser()` function as the composition root: combine named mini parser functions in source-grammar order so its body reads as the construct's high-level grammar. Move reusable recognition and recovery into the owning grammar module; move declaration-specific alternatives into focused mini parsers. Inline a parser only when naming it would add no source-language meaning.
2. Show the exact user-source or grammar region each mini parser owns. Name project terms concretely—for example, show that the table header is `User schemafull` in `table | User schemafull | {`, including what the parser excludes.
3. State what tokens or source region it consumes, what value it emits, what recovery values mean, and whether or when it mutates parser state or the AST.
4. Document private staged carrier types when their fields encode recovery, provenance, ordering, or commit state that the type name does not reveal.
5. Explain the shared Chumsky signature once in each non-trivial module, or point to an enclosing explanation: `'src` owns borrowed source spelling, `'tokens` owns the token input, `'src: 'tokens` keeps spelling alive during parsing, and `impl Parser` is a parser definition rather than parsed output.
6. Trace one representative accepted input through important intermediate values to its AST result. When directed recovery exists, also trace one malformed input to its selected problem and say whether partial AST data can exist.
7. Explain `choice` ordering and combinators such as `rewind`, `ignored`, or `map_with` wherever changing or simplifying them could alter consumption, diagnostics, spans, or construction timing.
8. Route table, field, and later declaration-name slots through `grammar::declared_name::parser`, supplying the parser for the declaration-specific syntax that follows the name. Keep only declaration-specific missing-tail or body recovery in the caller.

Finish only when a reader can answer, from the changed file, “what source construct does this function own, what does it consume and return, and when does it commit?” Keep linear token selectors and self-explanatory wiring concise.
