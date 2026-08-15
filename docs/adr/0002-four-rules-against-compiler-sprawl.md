# Four rules against compiler sprawl

Two previous attempts at Aureline were abandoned not because the semantics were wrong but because the code became unnavigable: adding a feature turned hellish, and changes started to feel like quick hacks. Attempt two reached ~12,800 lines across its AST, parser, and semantic crates without being able to check a single query — capability grew far slower than code. These four rules exist to keep the third attempt's middle layer from sprawling the same way, and they bind every slice.

1. **One walk builds, everything else reads.** Syntax is walked exactly once, into an index. Checks query that index; no check re-walks the syntax tree.
2. **Separate types for separate roles.** Syntax AST, semantic index, and generation artifact are distinct types. When one needs a field to serve another's job, that is the signal to add a seam — not a field.
3. **No silent fallbacks.** Unsupported syntax always produces a diagnostic plus an explicit `Error` or `Unknown`, never a quiet `Any`.
4. **Build infrastructure only when a feature needs it.** No expression checker before there are expressions; no pass registry before passes repeat; no plugin API before there are plugins.

## Considered options

The alternative was the informal discipline used in both previous attempts — no stated structural rules, relying on judgement to notice the drift. That failed twice, and in both cases the mess was recognised only well after it had set. A heavier alternative was also rejected: formal tripwires such as per-file size budgets or a measured change-amplification test at each slice boundary. Those were considered and deliberately not adopted; the three symptoms to watch for are recorded on the Wayfinder map instead, and detection stays a matter of judgement.

## Consequences

Rule 1 is what attempt two most clearly violated: two exhaustive walks over the same large expression enum (a 363-line reference walker plus a second full dispatch in inference), so every new syntax node required edits in several places. Rule 2 addresses that attempt's single expression type serving simultaneously as syntax tree, query IR, and traversal substrate. Rule 3 forbids the lossy type lowering that quietly collapsed object shapes and unsupported forms to `Any`, which both hid errors and undermined the strict checking Aureline exists to provide.

Rule 4 has an immediate and counter-intuitive cost worth stating plainly: it forbids building the expression type checker during the table slice, because a table declaration contains no expressions. The bidirectional `infer`/`check` design is the intended destination, but it is not to be built until the first expression arrives with attributes. Building the general mechanism ahead of the feature that needs it is the specific mistake that produced the parser-first sprawl of the earlier attempts.
