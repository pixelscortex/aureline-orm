# Aureline

Aureline is a greenfield ORM/tooling workspace for a write-once, connect-everywhere model. Rust owns the schema language, compiler frontend, semantic analysis, and code generation. Language SDKs consume generated artifacts for each runtime ecosystem.

## Current scaffold

```text
.
├── .moon/
│   ├── toolchain.yml      # moon/proto toolchain config: Node + pnpm + Rust
│   └── workspace.yml      # moon project discovery for Rust, SDK, site, and example projects
├── core/
│   ├── ast/               # shared syntax tree and language data structures
│   ├── parser/            # .aurl parser entrypoints
│   ├── checker/           # semantic checker and diagnostics
│   ├── migration/         # migration preview primitives
│   ├── wasm/              # browser WASM wrapper over parser/checker APIs as needed
│   └── cli/               # Rust CLI entrypoint
├── testing/
│   └── contract/          # private Rust contract-test harness and suites
├── sdks/
│   └── js/                # TypeScript SDK package; package: @aureline/js
├── site/                  # SvelteKit site for docs + playground, initialized with shadcn-svelte preset
├── Cargo.toml             # Cargo workspace
├── package.json           # pnpm workspace root scripts delegating to moon
└── pnpm-workspace.yaml    # Node workspace discovery
```

## Why this shape

- `core/`: Rust-owned compiler system. Parser, semantic analysis, diagnostics, codegen, CLI, and WASM compiler wrapper belong here.
- `testing/`: Non-published Rust contract infrastructure and suites. It depends on compiler crates; production crates never depend on it.
- `sdks/`: publishable language SDK/runtime packages only. JavaScript, Python, Rust, Go, and future SDKs belong here.
- `site/`: public website surface. Keep docs and playground together here unless they become operationally different products later.
- `examples/`: future examples, fixtures, and tutorial projects. Moon and pnpm already reserve `examples/*`.

This avoids a common mistake: putting codegen under SDKs. Codegen is compiler infrastructure because it emits every SDK. SDK directories should contain runtime/client packages, not the generator that creates them.

Naming rule: package/crate/project IDs use the `aureline-` prefix (or npm scope `@aureline/*`). Directory names stay short (`core/ast`, `sdks/js`, `site`) because the parent folder already supplies the context.

## Current project names

- Rust AST package: `aureline-ast`
- Rust parser package: `aureline-parser`
- Rust checker package: `aureline-checker`
- Rust migration package: `aureline-migration`
- Rust WASM package: `aureline-wasm`
- Rust CLI package: `aureline-cli`
- Rust contract-test package: `aureline-test` (private)
- JS SDK package: `@aureline/js`
- SvelteKit site package: `@aureline/site`

`aureline-core` was intentionally removed. The name was too vague; crates now depend directly on the compiler packages they actually use.

## Core language split

The current Rust language boundary is:

```text
core/
├── ast/         # shared syntax tree and language data structures
├── parser/      # parses source into ast/data structures
├── checker/     # semantic diagnostics over parsed structures
├── migration/   # migration preview/planning primitives
├── wasm/        # browser wrapper; depends directly on compiler crates it exposes
└── cli/         # command-line UX; depends directly on compiler crates it uses
```

Dependency rule:

```text
wasm -> parser
cli  -> ast/parser/checker/migration as needed
```

The CLI and WASM wrapper may depend directly on `ast`, `parser`, `checker`, or `migration`; each package should expose only the compiler stages it actually needs.

Future compiler-side crates should fit this same rule:

```text
core/
├── codegen/      # language-neutral generator orchestration and target emitters
└── formatter/    # future .aurl formatter
```

Only split further when a real boundary appears. Empty facades are noise; small direct crates keep dependencies obvious.

## Future SDK split

Recommended SDK/runtime homes:

```text
sdks/
├── js/            # npm package: @aureline/js
├── python/        # Python package with pyproject.toml
├── rust/          # Rust client/runtime crate, if publishable separately
└── go/            # Go module, if/when Go becomes a target
```

Rules:

- SDKs own runtime ergonomics and generated client APIs.
- SDKs should not own parser, analyzer, or generator logic.
- The Rust SDK should live in `sdks/rust` if it is a user-facing generated client/runtime. Compiler crates stay in `core/`.

## Site and playground

The site is the right place for docs and the browser playground together:

```text
site/
├── src/
│   ├── lib/          # shared UI, editor components, playground panels, client state
│   ├── routes/       # docs and playground routes
│   └── app.html
├── static/           # static assets/examples only
├── components.json   # shadcn-svelte config
├── package.json
├── tsconfig.json
└── vite.config.ts
```

SvelteKit must be scaffolded before shadcn-svelte is initialized. The order used for this repo is:

```sh
pnpm dlx sv create --template minimal --types ts --no-add-ons --no-install --no-dir-check site
(cd site && pnpm dlx sv add tailwindcss="plugins:none" --install pnpm --no-git-check)
(cd site && pnpm dlx shadcn-svelte init --preset bdzH1zlY8 --overwrite --css src/routes/layout.css --lib-alias '$lib' --components-alias '$lib/components' --utils-alias '$lib/utils' --hooks-alias '$lib/hooks' --ui-alias '$lib/components/ui')
```

The shadcn-svelte command must run from `site/` after Tailwind is present.

## WASM boundary

Use `core/wasm`, not `site/` and not `sdks/js`, for the browser compiler wrapper.

Reasoning:

- `core/wasm` is a Rust compiler boundary exposed through `wasm-bindgen`.
- `site/` consumes the WASM package and renders the UI.
- `sdks/js` is the JavaScript runtime/client package, not the compiler frontend.

First playground API should be one stable function:

```ts
inspectAurl(source: string): InspectResult
```

Return JSON-friendly DTOs, not raw Rust ASTs:

```ts
type InspectResult = {
  ok: boolean;
  parseDiagnostics: Diagnostic[];
  semanticDiagnostics: Diagnostic[];
  document?: DocumentSummary;
  schema?: SchemaSummary;
  operations?: OperationSummary[];
  loweredSurql?: LoweredSurqlSummary[];
};
```

Browser scope:

- Good: parse `.aurl`, parse embedded SurQL, lower to IR, run semantic checks, format diagnostics, preview SDK APIs, preview migration text.
- Not needed: SurrealDB engine, storage, auth, network drivers, real query execution, migration application.

## Commands

```sh
pnpm install --no-frozen-lockfile
moon run :check
moon run :test
moon run :build
moon run :format
```
