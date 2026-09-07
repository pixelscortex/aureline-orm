# Aureline playground

From the repository root, install the workspace dependencies with `pnpm install`, then run:

```sh
pnpm playground
```

This builds the current Rust compiler wrapper with `wasm-pack` and starts SvelteKit. Open the printed local URL at `/playground`. Rust, the `wasm32-unknown-unknown` target, and `wasm-pack` are required; the build installs the target when Rustup is available.

The editor parses after 300 ms of inactivity. Try the table, composite-type, and malformed-input examples, or enter an empty document. Successful output shows tables and fields in source order, exact field type spelling, and UTF-8 byte spans. Invalid syntax returns the parser's structured phase-local problems and no tables. Type names, record targets, and duplicates are not semantically checked yet.

The output is an experimental inspection view, not a complete serialized AST or the future versioned Diagnostic envelope. The source is parsed locally in WebAssembly.

To use the wrapper in another browser-mounted Svelte component:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import init, { parse } from '@aureline/wasm';

  let output = $state('Loading…');

  onMount(() => {
    init()
      .then(() => {
        output = JSON.stringify(parse('table User schemafull { name string }'), null, 2);
      })
      .catch((error) => {
        output = String(error);
      });
  });
</script>

<pre>{output}</pre>
```

Svelte edits hot-reload. After changing Rust, stop the server and rerun `pnpm playground` to rebuild the WASM package. Generated `core/wasm/dist` files are ignored by Git.

To check and build the site after generating WASM:

```sh
pnpm --filter @aureline/site check
pnpm --filter @aureline/site build
```
