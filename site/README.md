# Aureline playground

From the repository root, install the workspace dependencies with `pnpm install`, then run:

```sh
pnpm playground
```

This builds the current Rust compiler wrapper with `wasm-pack` and starts SvelteKit. Open the printed local URL at `/playground`. Rust, the `wasm32-unknown-unknown` target, and `wasm-pack` are required; the build installs the target when Rustup is available.

The editor checks after 300 ms of inactivity. Try the table, composite-type, and malformed-input examples, or enter an empty document. The Semantic view shows canonical field types and required/optional presence in source order. Syntax-invalid input is separated from semantic-invalid input; both retain ordered problems and UTF-8 byte spans. Lexer and AST views expose the corresponding syntax inspection data.

The output is an experimental inspection view, not a complete serialized AST or a versioned Diagnostic envelope. The source is parsed and checked locally in WebAssembly.

To use the wrapper in another browser-mounted Svelte component:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import init, { check } from '@aureline/wasm';

  let output = $state('Loading…');

  onMount(() => {
    init()
      .then(() => {
        output = JSON.stringify(check('table User schemafull { id string }'), null, 2);
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
