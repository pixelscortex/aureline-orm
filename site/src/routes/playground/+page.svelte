<script lang="ts">
	import { onMount } from "svelte";
	import { Debounced } from "runed";
	import init, { parse } from "@aureline/wasm";

	type SourceSpan = { source: number; range: { start: number; end: number } };
	type ParsedTable = {
		name: string;
		schemaMode: "schemafull" | "schemaless";
		span: SourceSpan;
		fields: { name: string; sourceType: string; span: SourceSpan }[];
	};
	type ParserResult =
		| { status: "parsed"; tables: ParsedTable[] }
		| { status: "invalid"; problems: unknown[] };

	const examples = [
		{ label: "Tables", source: `table User schemafull {
    name string
    email option<string>
    tags array<string>
}

table Post schemafull {
    title string
    author record<User>
}` },
		{ label: "Composite types", source: `// Types preserve the syntax you write.
table Event schemaless {
    id [string, int]
    owner record<User | Bot>
    coordinates array<float, 3>
    payload FutureType
}` },
		{ label: "Try an error", source: `table User schemafull {
    first name string
}` }
	];
	let source = $state(examples[0].source);
	let result = $state<ParserResult | null>(null);
	let runtimeError = $state<string | null>(null);
	let wasmReady = $state(false);
	const debouncedSource = new Debounced(() => source, 300);

	function runParser(value: string) {
		try {
			runtimeError = null;
			result = parse(value) as ParserResult;
		} catch (error) {
			result = null;
			runtimeError = error instanceof Error ? error.message : String(error);
		}
	}

	onMount(async () => {
		try {
			await init();
			wasmReady = true;
			runParser(source);
		} catch (error) {
			runtimeError = error instanceof Error ? error.message : String(error);
		}
	});

	$effect(() => {
		const value = debouncedSource.current;
		if (wasmReady) runParser(value);
	});
</script>

<svelte:head>
	<title>Aureline parser playground</title>
	<meta name="description" content="Parse Aureline table schemas in your browser with WebAssembly." />
</svelte:head>

<main class="mx-auto max-w-5xl px-4 py-12 sm:px-6 sm:py-20">
	<section class="mb-10 max-w-2xl">
		<p class="mb-3 text-xs font-bold uppercase tracking-[0.12em] text-muted-foreground">Aureline / WebAssembly</p>
		<h1 class="mb-3 text-5xl font-bold tracking-[-0.06em] sm:text-7xl font-veloce">Parser playground</h1>
		<p class="text-lg leading-relaxed text-muted-foreground">Write table schemas and inspect their parsed tables and fields. Everything runs locally in your browser.</p>
		<p class="mt-3 text-sm text-muted-foreground">Syntax only: type names, record targets, and duplicate declarations are not checked yet.</p>
	</section>

	<section class="mb-4 rounded-xl border border-border bg-card p-5 shadow-xl shadow-foreground/5" aria-label="Parser playground">
		<div class="mb-4 flex flex-wrap gap-2" aria-label="Examples">
			{#each examples as example}
				<button type="button" class="rounded-md border border-border px-3 py-1.5 text-sm hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring" onclick={() => source = example.source}>{example.label}</button>
			{/each}
		</div>
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
			{#if runtimeError}
				Parser unavailable. See the error below.
			{:else if !wasmReady}
				Loading parser…
			{:else}
				Updates after 300ms of inactivity.
			{/if}
		</p>
	</section>

	<section class="rounded-xl border border-border bg-card p-5 shadow-xl shadow-foreground/5" aria-live="polite">
		<div class="mb-3 flex items-center justify-between gap-4">
			<h2 class="text-base font-bold">Parser output</h2>
			{#if result?.status === "invalid"}
				<span class="rounded-full bg-destructive/10 px-2.5 py-1 text-xs font-bold text-destructive">{result.problems.length} {result.problems.length === 1 ? 'error' : 'errors'}</span>
			{:else if result?.status === "parsed"}
				<span class="rounded-full bg-green-500/10 px-2.5 py-1 text-xs font-bold text-green-700 dark:text-green-300">Syntax valid · {result.tables.length} {result.tables.length === 1 ? 'table' : 'tables'}</span>
			{/if}
		</div>

		{#if runtimeError}
			<pre class="min-h-24 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-destructive/10 p-4 font-mono text-sm leading-relaxed text-destructive">{runtimeError}</pre>
		{:else if result?.status === "invalid"}
			<pre class="min-h-24 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-destructive/10 p-4 font-mono text-sm leading-relaxed text-destructive">{JSON.stringify(result.problems, null, 2)}</pre>
		{:else if result?.status === "parsed"}
			<pre class="min-h-24 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-muted/50 p-4 font-mono text-sm leading-relaxed text-foreground">{JSON.stringify(result.tables, null, 2)}</pre>
		{:else}
			<pre class="min-h-24 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-muted/50 p-4 font-mono text-sm leading-relaxed text-muted-foreground">Waiting for the parser…</pre>
		{/if}
	</section>
</main>
