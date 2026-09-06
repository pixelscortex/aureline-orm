# Parser contract tests

Keep each parser contract in the module matching its grammar concern, and preserve the test function's descriptive name when moving or splitting cases. Prefer assertions that describe the public AST or typed diagnostic being protected. Add a comment only for a non-obvious parser invariant or design decision; follow [comments-guidelines](../../../.agents/skills/comments-guidelines/SKILL.md) when deciding whether a comment earns its place.
