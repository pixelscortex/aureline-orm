<script lang="ts">
	import { generate_migration } from "@aureline/wasm";

	let { source, ready }: { source: string; ready: boolean } = $props();
	type Warning = { kind: string; consequence: string; table: string; field?: string };
	type Generated = { status: "generated"; script: string; snapshot: string; warnings: Warning[]; operationCount: number };
	type Result = Generated | { status: "unchanged" } | { status: "invalid"; phase: string; messages: string[] };
	type Entry = Generated & { source: string };
	let history = $state<Entry[]>([]);
	let selected = $state(0);
	let artifact = $state<"script" | "snapshot" | "source">("script");
	let notice = $state<{ source: string; error: boolean; messages: string[] } | null>(null);
	let generating = $state(false);
	// `latest` is the comparison base; `entry` is only the artifact currently being viewed.
	// Keeping those roles separate lets users inspect an older migration without changing the next diff.
	const latest = $derived(history.at(-1));
	const entry = $derived(history[selected]);
	const edited = $derived(latest !== undefined && latest.source !== source);

	/**
	 * Records a generated migration in this browser session. A no-op, validation error, or
	 * exception leaves history untouched, so the next attempt still compares with the same snapshot.
	 */
	function generate() {
		if (!ready || generating) return;
		generating = true;
		try {
			// Only a successful, nonempty migration advances the comparison base.
			const result = generate_migration(source, latest?.snapshot) as Result;
			if (result.status === "generated") {
				history = [...history, { ...result, source }];
				selected = history.length - 1;
				artifact = "script";
				notice = { source, error: false, messages: [`Migration ${String(history.length).padStart(2, "0")} saved in this session.`] };
			} else if (result.status === "unchanged") {
				notice = { source, error: false, messages: ["No schema changes. No snapshot added."] };
			} else {
				notice = { source, error: true, messages: ["Generation blocked. History is unchanged.", ...result.messages] };
			}
		} catch (error) {
			notice = { source, error: true, messages: ["Could not generate. History is unchanged.", error instanceof Error ? error.message : String(error)] };
		} finally {
			generating = false;
		}
	}

	function reset() {
		history = [];
		selected = 0;
		artifact = "script";
		notice = null;
	}
</script>

<div class="space-y-5">
	<div class="flex flex-wrap items-start justify-between gap-3">
		<div class="min-w-0 flex-1">
			<h3 class="text-sm font-bold">Make a change. Keep the history.</h3>
			<p class="mt-1 text-xs leading-relaxed text-muted-foreground">Generate from your schema, edit it, then generate again to see what changed.</p>
		</div>
		<button type="button" onclick={generate} disabled={!ready || generating} class="shrink-0 rounded-lg bg-foreground px-4 py-2 text-sm font-bold text-background transition hover:opacity-85 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-wait disabled:opacity-40">{generating ? "Generating…" : "Generate"}</button>
	</div>

	<p class="text-[11px] leading-relaxed text-muted-foreground">Browser session only · refresh clears history · nothing runs on a database.</p>

	<!-- A notice belongs to the source that produced it; hide stale feedback after a new edit. -->
	{#if notice && notice.source === source}
		<div role="status" class={`rounded-lg border p-3 text-xs leading-relaxed ${notice.error ? "border-destructive/20 bg-destructive/5 text-destructive" : "border-border bg-muted/30 text-foreground"}`}>
			{#each notice.messages as message}<p class="break-words">{message}</p>{/each}
		</div>
	{/if}

	{#if history.length === 0}
		<div class="border-y border-dashed border-border py-10 text-center">
			<p class="font-mono text-xs text-muted-foreground">EMPTY HISTORY → YOUR FIRST MIGRATION</p>
			<p class="mt-3 text-sm font-semibold">Start with a snapshot of your schema.</p>
			<p class="mx-auto mt-2 max-w-xs text-xs leading-relaxed text-muted-foreground">Click Generate, then try adding <code class="font-mono text-foreground">email option&lt;string&gt;</code> to a table. Your next migration will contain just that change.</p>
		</div>
	{:else}
		<div>
			<div class="mb-2 flex items-center justify-between gap-3">
				<h3 class="text-xs font-bold">History <span class="ml-1 font-normal text-muted-foreground">{history.length}</span></h3>
				<button type="button" onclick={reset} class="rounded px-2 py-1 text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">Reset history</button>
			</div>
			<!-- History entries are immutable session records; selecting one changes only the displayed artifact. -->
			<div class="flex gap-2 overflow-x-auto pb-2" aria-label="Migration history">
				{#each history as migration, index}
					<button type="button" aria-pressed={selected === index} onclick={() => (selected = index)} class={`shrink-0 rounded-lg border px-3 py-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${selected === index ? "border-foreground bg-muted" : "border-border hover:bg-muted/40"}`}>
						<span class="block text-xs font-bold">Migration {String(index + 1).padStart(2, "0")}{index === history.length - 1 ? " · latest" : ""}</span>
						<span class="mt-1 block text-[10px] text-muted-foreground">{migration.operationCount} {migration.operationCount === 1 ? "operation" : "operations"}{migration.warnings.length ? ` · ${migration.warnings.length} warnings` : ""}</span>
					</button>
				{/each}
			</div>
			<p class="mt-1 text-[11px] leading-relaxed text-muted-foreground">Generate always compares with migration {String(history.length).padStart(2, "0")}.{edited ? " Your editor has changed since then." : ""}</p>
		</div>
	{/if}

	{#if entry}
		<div class="min-w-0">
			<div class="mb-3 flex flex-wrap items-center justify-between gap-2">
				<p class="text-xs font-bold">Migration {String(selected + 1).padStart(2, "0")}</p>
				<!-- These views expose the exact script, snapshot, and source captured by this history entry. -->
				<div class="flex gap-1" aria-label="Migration artifacts">
					{#each [{ id: "script" as const, label: "Script" }, { id: "snapshot" as const, label: "Snapshot" }, { id: "source" as const, label: "Saved schema" }] as view}
						<button type="button" aria-pressed={artifact === view.id} onclick={() => (artifact = view.id)} class={`rounded px-2 py-1 text-[11px] font-semibold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${artifact === view.id ? "bg-foreground text-background" : "text-muted-foreground hover:bg-muted"}`}>{view.label}</button>
					{/each}
				</div>
			</div>
			{#if entry.warnings.length > 0}
				<p class="mb-3 rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 text-xs leading-relaxed text-amber-900 dark:text-amber-100">{entry.warnings.length} {entry.warnings.length === 1 ? "warning" : "warnings"} · This change may invalidate stored values or remove data. Review the warning header in the script.</p>
			{/if}
			<pre aria-label={artifact === "script" ? "Migration script" : artifact === "snapshot" ? "Migration snapshot" : "Saved migration schema"} class="overflow-x-auto rounded-xl bg-muted/40 p-4 font-mono text-xs leading-relaxed text-foreground">{artifact === "script" ? entry.script : artifact === "snapshot" ? entry.snapshot : entry.source}</pre>
		</div>
	{/if}
</div>
