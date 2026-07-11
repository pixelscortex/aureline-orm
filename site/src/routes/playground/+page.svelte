<script lang="ts">
	import { onMount } from "svelte";
	import { Debounced } from "runed";
	import init, { lexer } from "@aureline/wasm";

	type LexerResult = {
		tokens: unknown[];
		errors: string[];
	};

	let source = $state("table{}");
	let result = $state<LexerResult | null>(null);
	let runtimeError = $state<string | null>(null);
	let wasmReady = $state(false);
	const debouncedSource = new Debounced(() => source, 300);

	function runLexer(value: string) {
		try {
			runtimeError = null;
			result = lexer(value) as LexerResult;
		} catch (error) {
			result = null;
			runtimeError = error instanceof Error ? error.message : String(error);
		}
	}

	onMount(async () => {
		try {
			await init();
			wasmReady = true;
			runLexer(source);
		} catch (error) {
			runtimeError = error instanceof Error ? error.message : String(error);
		}
	});

	$effect(() => {
		const value = debouncedSource.current;
		if (wasmReady) runLexer(value);
	});
</script>

<svelte:head>
	<title>Aureline lexer playground</title>
	<meta name="description" content="Try the Aureline lexer in your browser." />
</svelte:head>

<main class="mx-auto max-w-5xl px-4 py-12 sm:px-6 sm:py-20">
	<section class="mb-10 max-w-2xl">
		<p class="mb-3 text-xs font-bold uppercase tracking-[0.12em] text-muted-foreground">Aureline / WebAssembly</p>
		<h1 class="mb-3 text-5xl font-bold tracking-[-0.06em] sm:text-7xl font-veloce">Lexer playground</h1>
		<p class="text-lg leading-relaxed text-muted-foreground">Type Aureline syntax and inspect the tokens produced by the browser lexer.</p>
	</section>

	<section class="mb-4 rounded-xl border border-border bg-card p-5 shadow-xl shadow-foreground/5" aria-label="Lexer playground">
		<label class="mb-2 block text-sm font-bold" for="source">Input</label>
		<textarea
			id="source"
			class="block min-h-56 w-full resize-y rounded-lg border border-input bg-background p-4 font-mono text-sm leading-relaxed text-foreground outline-none transition focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50"
			bind:value={source}
			spellcheck="false"
			aria-describedby="debounce-status"
			placeholder="Type Aureline syntax…"
		></textarea>
		<p id="debounce-status" class="mt-2 text-xs text-muted-foreground">
			{#if !wasmReady}
				Loading lexer…
			{:else}
				Updates after 300ms of inactivity.
			{/if}
		</p>
	</section>

	<section class="rounded-xl border border-border bg-card p-5 shadow-xl shadow-foreground/5" aria-live="polite">
		<div class="mb-3 flex items-center justify-between gap-4">
			<h2 class="text-base font-bold">Lexer output</h2>
			{#if result?.errors.length}
				<span class="rounded-full bg-destructive/10 px-2.5 py-1 text-xs font-bold text-destructive">{result.errors.length} {result.errors.length === 1 ? 'error' : 'errors'}</span>
			{:else if result}
				<span class="rounded-full bg-green-500/10 px-2.5 py-1 text-xs font-bold text-green-700 dark:text-green-300">{result.tokens.length} {result.tokens.length === 1 ? 'token' : 'tokens'}</span>
			{/if}
		</div>

		{#if runtimeError}
			<pre class="min-h-24 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-destructive/10 p-4 font-mono text-sm leading-relaxed text-destructive">{runtimeError}</pre>
		{:else if result?.errors.length}
			<pre class="min-h-24 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-destructive/10 p-4 font-mono text-sm leading-relaxed text-destructive">{JSON.stringify(result.errors, null, 2)}</pre>
		{:else if result}
			<pre class="min-h-24 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-muted/50 p-4 font-mono text-sm leading-relaxed text-foreground">{JSON.stringify(result.tokens, null, 2)}</pre>
		{:else}
			<pre class="min-h-24 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-muted/50 p-4 font-mono text-sm leading-relaxed text-muted-foreground">Waiting for the lexer…</pre>
		{/if}
	</section>
</main>
