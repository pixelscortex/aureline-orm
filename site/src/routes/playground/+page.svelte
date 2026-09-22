<script lang="ts">
	import { onMount } from "svelte";
	import { Debounced } from "runed";
	import MigrationPanel from "$lib/components/MigrationPanel.svelte";
	import init, { check, lexer, parse } from "@aureline/wasm";

	type SourceSpan = { source: number; range: { start: number; end: number } };
	type ParsedTable = {
		name: string;
		schemaMode: "schemafull" | "schemaless";
		span: SourceSpan;
		fields: { name: string; sourceType: string; span: SourceSpan }[];
	};
	type ParserResult =
		| { status: "parsed"; tables: ParsedTable[] }
		| { status: "invalid"; problems: SyntaxProblem[] };
	type LexerResult = { tokens: unknown[]; errors: unknown[] };
	type CheckedField = {
		name: string;
		sourceType: string;
		semanticType: string;
		presence: "required" | "optional";
		span: SourceSpan;
	};
	type CheckedTable = {
		name: string;
		schemaMode: "schemafull" | "schemaless";
		span: SourceSpan;
		fields: CheckedField[];
	};
	type SyntaxProblem = { [kind: string]: unknown };
	type SemanticProblem = {
		kind:
			| "duplicateTable"
			| "duplicateField"
			| "unknownType"
			| "unsupportedType"
			| "bareTableType"
			| "wrongArity"
			| "missingRecordTarget"
			| "ambiguousRecordTarget"
			| "wrongArgumentRole"
			| "invalidCollectionSize"
			| "invalidRecordKey";
		message: string;
		span: SourceSpan;
		firstSpan?: SourceSpan;
		table?: string;
		name?: string;
		field?: string;
		minimum?: number;
		maximum?: number;
		actual?: number;
		candidates?: Array<{ name: string; span: SourceSpan }>;
		position?: number;
		expected?: string;
		raw?: string;
	};
	type CheckResult =
		| { status: "checked"; tables: CheckedTable[] }
		| { status: "invalid"; phase: "syntax"; problems: SyntaxProblem[] }
		| { status: "invalid"; phase: "semantic"; problems: SemanticProblem[] };
	type Tab = "semantic" | "lexer" | "ast" | "migration";

	const examples = [
		{ label: "Migration demo", source: `table User schemafull {
	id string
	name string
	tags set<string, 3>
}` },
		{
			label: "A valid schema",
			source: `table User schemafull {
	id [string, int]
	name string
	email option<string>
	tags array<string>
	scores set<int, 3>
}

table Bot schemaless {
	id string
}

table Event schemaless {
	id string
	owner record<User | Bot>
	coordinates array<float, 3>
	payload option<array<string>>
}`
		},
		{
			label: "Semantic errors",
			source: `table User schemafull {
	id string
	owner record<MissingTable>
	payload FutureType
}`
		},
		{
			label: "Syntax error",
			source: `table User schemafull {
	first name string
}`
		}
	];

	let source = $state(examples[0].source);
	let parserResult = $state<ParserResult | null>(null);
	let lexerResult = $state<LexerResult | null>(null);
	let semanticResult = $state<CheckResult | null>(null);
	let runtimeError = $state<string | null>(null);
	let wasmReady = $state(false);
	let activeTab = $state<Tab>("migration");
	const debouncedSource = new Debounced(() => source, 300);

	/**
	 * Runs the read-only compiler inspection shown in the Lexer, AST, and Semantic tabs.
	 * Migration history is intentionally excluded: it advances only when the user presses
	 * Generate in MigrationPanel, so editing the source cannot silently create a migration.
	 */
	function runInspection(value: string) {
		try {
			runtimeError = null;
			parserResult = parse(value) as ParserResult;
			lexerResult = lexer(value) as LexerResult;
			// The checker owns meaning. The page only chooses how to present its DTO.
			semanticResult = check(value) as CheckResult;
		} catch (error) {
			parserResult = null;
			lexerResult = null;
			semanticResult = null;
			runtimeError = error instanceof Error ? error.message : String(error);
		}
	}

	onMount(async () => {
		try {
			await init();
			wasmReady = true;
			runInspection(source);
		} catch (error) {
			runtimeError = error instanceof Error ? error.message : String(error);
		}
	});

	$effect(() => {
		// Debouncing keeps the WASM inspection responsive while the user is typing; the
		// explicit Generate action remains the only way to record migration history.
		const value = debouncedSource.current;
		if (wasmReady) runInspection(value);
	});

	function selectTab(tab: Tab) {
		activeTab = tab;
	}

	function handleTabKeydown(event: KeyboardEvent) {
		// Keep one tab in the roving tab order and move focus with the standard tab-list keys.
		const tabs: Tab[] = ["lexer", "ast", "semantic", "migration"];
		const current = tabs.indexOf(activeTab);
		let next = current;
		if (event.key === "ArrowRight" || event.key === "ArrowDown") next = (current + 1) % tabs.length;
		if (event.key === "ArrowLeft" || event.key === "ArrowUp") next = (current - 1 + tabs.length) % tabs.length;
		if (event.key === "Home") next = 0;
		if (event.key === "End") next = tabs.length - 1;
		if (next !== current) {
			event.preventDefault();
			activeTab = tabs[next];
			(document.getElementById(`tab-${tabs[next]}`) as HTMLButtonElement | null)?.focus();
		}
	}

	function isParsed(): boolean {
		return parserResult?.status === "parsed";
	}

	function semanticStatus(): string {
		return semanticResult?.status ?? "pending";
	}

	function semanticPhase(): string {
		return semanticResult?.status === "invalid" ? semanticResult.phase : "";
	}

	function syntaxIsInvalid(): boolean {
		return parserResult?.status === "invalid" || (semanticStatus() === "invalid" && semanticPhase() === "syntax");
	}

	function semanticIsInvalid(): boolean {
		return semanticStatus() === "invalid" && semanticPhase() === "semantic";
	}

	function semanticTables(): CheckedTable[] {
		return semanticResult?.status === "checked" ? semanticResult.tables : [];
	}

	function semanticFields(): Array<CheckedField & { table: string }> {
		return semanticTables().flatMap((table) => table.fields.map((field) => ({ table: table.name, ...field })));
	}

	function semanticFindings(): SemanticProblem[] {
		return semanticResult?.status === "invalid" && semanticResult.phase === "semantic" ? semanticResult.problems : [];
	}

	function findingSpan(finding: SemanticProblem): string {
		return `bytes ${finding.span.range.start}–${finding.span.range.end}`;
	}

	function findingSecondarySpan(finding: SemanticProblem): string | null {
		if (!finding.firstSpan) return null;
		return `first declaration · bytes ${finding.firstSpan.range.start}–${finding.firstSpan.range.end}`;
	}

	function json(value: unknown): string {
		return JSON.stringify(value, null, 2);
	}

	function syntaxProblemLabel(problem: SyntaxProblem): string {
		const kind = Object.keys(problem)[0];
		return kind ? kind.replace(/[A-Z]/g, (letter) => ` ${letter.toLowerCase()}`) : "Syntax problem";
	}
</script>

<svelte:head>
	<title>Aureline playground</title>
	<meta name="description" content="Explore Aureline schemas and generate migrations in your browser." />
</svelte:head>

<main class="mx-auto w-full max-w-[1440px] px-4 pb-10 pt-28 sm:px-6 lg:px-10">
	<section class="mb-7 flex flex-col gap-5 sm:mb-8 sm:flex-row sm:items-end sm:justify-between">
		<div class="max-w-2xl">
			<p class="mb-2 text-xs font-bold uppercase tracking-[0.16em] text-muted-foreground">Aureline / browser compiler</p>
			<h1 class="font-veloce text-5xl font-bold tracking-[-0.06em] sm:text-7xl">Playground</h1>
			<p class="mt-3 max-w-xl text-base leading-relaxed text-muted-foreground sm:text-lg">
				Write a schema, generate a migration, then change it and watch your history grow.
			</p>
		</div>
		<div class="flex flex-wrap gap-2" aria-label="Examples">
			{#each examples as example}
				<button
					type="button"
					class="rounded-full border border-border bg-card px-3 py-1.5 text-xs font-bold text-foreground transition hover:border-foreground/40 hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
					onclick={() => (source = example.source)}
				>
					{example.label}
				</button>
			{/each}
		</div>
	</section>

	<section class="grid min-h-[680px] grid-cols-1 gap-4 lg:h-[max(620px,calc(100dvh-250px))] lg:min-h-0 lg:grid-cols-2" aria-label="Aureline playground">
		<article class="flex min-h-[560px] min-w-0 flex-col overflow-hidden rounded-2xl border border-border bg-card shadow-xl shadow-foreground/5 lg:min-h-0">
			<div class="flex items-center justify-between gap-3 border-b border-border px-4 py-3 sm:px-5">
				<div>
					<h2 class="text-sm font-bold">Source</h2>
					<p class="mt-0.5 text-xs text-muted-foreground">Aureline schema</p>
				</div>
				<span class="rounded-full bg-muted px-2.5 py-1 font-mono text-[11px] text-muted-foreground">.aurl</span>
			</div>
			<label class="sr-only" for="source">Aureline source</label>
			<textarea
				id="source"
				class="min-h-[480px] flex-1 resize-none bg-background/70 p-5 font-mono text-[13px] leading-7 text-foreground outline-none placeholder:text-muted-foreground/50 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/60 sm:p-6 sm:text-sm"
				bind:value={source}
				spellcheck="false"
				aria-describedby="source-status"
				placeholder="Type Aureline syntax…"
			></textarea>
			<div id="source-status" class="flex min-h-11 items-center justify-between gap-3 border-t border-border px-4 py-3 text-xs text-muted-foreground sm:px-5">
				<span>
					{#if runtimeError}
						Compiler unavailable
					{:else if !wasmReady}
						Loading compiler…
					{:else}
						Live · updates after 300ms
					{/if}
				</span>
				<span class="font-mono">{source.length} chars</span>
			</div>
		</article>

		<article class="flex min-h-[560px] min-w-0 flex-col overflow-hidden rounded-2xl border border-border bg-card shadow-xl shadow-foreground/5 lg:min-h-0" aria-live="polite">
			<div class="border-b border-border px-4 pt-3 sm:px-5">
				<div class="flex items-center justify-between gap-3">
					<div>
						<h2 class="text-sm font-bold">Output</h2>
						<p class="mt-0.5 text-xs text-muted-foreground">Live inspection and saved migrations</p>
					</div>
					{#if semanticStatus() === "checked"}
						<span class="rounded-full bg-green-500/10 px-2.5 py-1 text-[11px] font-bold text-green-700 dark:text-green-300">Semantics valid</span>
					{:else if semanticIsInvalid()}
						<span class="rounded-full bg-destructive/10 px-2.5 py-1 text-[11px] font-bold text-destructive">Needs attention</span>
					{:else if syntaxIsInvalid()}
						<span class="rounded-full bg-amber-500/10 px-2.5 py-1 text-[11px] font-bold text-amber-700 dark:text-amber-300">Syntax invalid</span>
					{/if}
				</div>
				<div class="mt-4 flex gap-1" role="tablist" aria-label="Inspection views" tabindex="-1" onkeydown={handleTabKeydown}>
					{#each [
						{ id: "lexer" as Tab, label: "Lexer", hint: "tokens" },
						{ id: "ast" as Tab, label: "AST", hint: "syntax tree" },
						{ id: "semantic" as Tab, label: "Semantic", hint: "checked meaning" },
							{ id: "migration" as Tab, label: "Migrations", hint: "schema history" }
					] as tab}
						<button
							type="button"
							role="tab"
							id={`tab-${tab.id}`}
							aria-selected={activeTab === tab.id}
							aria-controls={`panel-${tab.id}`}
							tabindex={activeTab === tab.id ? 0 : -1}
							class={`border-b-2 px-3 pb-3 pt-1 text-left transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${activeTab === tab.id ? "border-foreground text-foreground" : "border-transparent text-muted-foreground hover:text-foreground"}`}
							onclick={() => selectTab(tab.id)}
						>
							<span class="block text-xs font-bold">{tab.label}</span>
							<span class="mt-0.5 hidden text-[10px] sm:block">{tab.hint}</span>
						</button>
					{/each}
				</div>
			</div>

			<div class="min-h-0 flex-1 overflow-auto p-4 sm:p-5">
				<!-- Keep this panel mounted while tabs change so its in-memory migration history survives inspection. -->
					<div id="panel-migration" class="scroll-mt-24" role="tabpanel" aria-labelledby="tab-migration" hidden={activeTab !== "migration"}>
						<MigrationPanel {source} ready={wasmReady} />
					</div>
				{#if runtimeError}
					<div class="rounded-xl border border-destructive/20 bg-destructive/10 p-4 text-sm text-destructive">
						<p class="font-bold">Compiler unavailable</p>
						<p class="mt-1 break-words font-mono text-xs">{runtimeError}</p>
					</div>
				{:else if activeTab === "semantic"}
					<div id="panel-semantic" role="tabpanel" aria-labelledby="tab-semantic">
						{#if syntaxIsInvalid()}
							<div class="rounded-xl border border-amber-500/20 bg-amber-500/10 p-4">
								<p class="text-sm font-bold text-amber-800 dark:text-amber-200">Syntax must be valid before semantics can run.</p>
								<p class="mt-1 text-xs leading-relaxed text-amber-800/75 dark:text-amber-200/75">Fix the parser problem, then the checker will report normalized fields here.</p>
							</div>
							{#if parserResult?.status === "invalid"}
								<div class="mt-4 space-y-2">
									{#each parserResult.problems as problem, index}
										<div class="flex gap-3 rounded-lg border border-border bg-muted/30 p-3 text-xs">
											<span class="font-mono text-muted-foreground">{String(index + 1).padStart(2, "0")}</span>
											<span>{syntaxProblemLabel(problem)}</span>
										</div>
									{/each}
								</div>
							{/if}
						{:else if semanticIsInvalid()}
							<div class="space-y-2">
								<p class="mb-3 text-xs text-muted-foreground">Ordered semantic findings · source spans are UTF-8 byte offsets</p>
								{#each semanticFindings() as finding, index}
									<div class="rounded-xl border border-destructive/15 bg-destructive/5 p-3">
										<div class="flex items-start gap-3">
											<span class="font-mono text-xs text-destructive/60">{String(index + 1).padStart(2, "0")}</span>
											<div class="min-w-0">
													<p class="text-sm font-semibold text-destructive">{finding.message}</p>
													<p class="mt-1 font-mono text-[11px] text-destructive/70">{findingSpan(finding)}</p>
													{#if findingSecondarySpan(finding)}<p class="mt-0.5 font-mono text-[11px] text-destructive/60">{findingSecondarySpan(finding)}</p>{/if}
													{#if finding.candidates}
														<div class="mt-2 space-y-1 border-t border-destructive/10 pt-2">
															<p class="text-[11px] font-semibold text-destructive/70">Candidates</p>
															{#each finding.candidates as candidate}
																<p class="font-mono text-[11px] text-destructive/60">{candidate.name} · bytes {candidate.span.range.start}–{candidate.span.range.end}</p>
															{/each}
														</div>
													{/if}
												</div>
										</div>
									</div>
								{:else}
									<p class="rounded-xl border border-border bg-muted/30 p-4 text-sm text-muted-foreground">No semantic findings returned.</p>
								{/each}
							</div>
						{:else if semanticStatus() === "checked"}
							<div class="space-y-3">
								<div class="flex items-center justify-between text-xs text-muted-foreground">
									<span>Normalized fields</span>
									<span>{semanticFields().length} {semanticFields().length === 1 ? "field" : "fields"}</span>
								</div>
								<div class="overflow-hidden rounded-xl border border-border">
									{#each semanticFields() as field}
										<div class="flex items-start justify-between gap-4 border-b border-border p-3 last:border-b-0">
											<div class="min-w-0 flex-1">
												<div class="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-0.5">
															<p class="truncate font-mono text-sm text-foreground">{field.name}</p>
													{#if field.table}<p class="text-[11px] text-muted-foreground">{String(field.table)}</p>{/if}
												</div>
													<p class="mt-2 break-words font-mono text-xs leading-relaxed text-foreground">{field.semanticType}</p>
													<p class="mt-1 break-words font-mono text-[11px] leading-relaxed text-muted-foreground/70">source · {field.sourceType}</p>
												</div>
											<span class="shrink-0 self-start rounded-full bg-muted px-2 py-0.5 text-[10px] font-bold uppercase tracking-wide text-muted-foreground">{field.presence}</span>
										</div>
									{:else}
										<p class="p-4 text-sm text-muted-foreground">No fields returned for this source.</p>
									{/each}
								</div>
							</div>
						{:else}
							<p class="rounded-xl border border-border bg-muted/30 p-4 text-sm text-muted-foreground">Waiting for the checker…</p>
						{/if}
					</div>
				{:else if activeTab === "lexer"}
					<div id="panel-lexer" role="tabpanel" aria-labelledby="tab-lexer">
						{#if lexerResult}
							<p class="mb-3 text-xs text-muted-foreground">Raw lexical tokens from the source.</p>
							<pre class="whitespace-pre-wrap break-words rounded-xl bg-muted/40 p-4 font-mono text-xs leading-relaxed text-foreground">{json(lexerResult)}</pre>
						{:else}
							<p class="rounded-xl border border-border bg-muted/30 p-4 text-sm text-muted-foreground">Waiting for the lexer…</p>
						{/if}
					</div>
				{:else if activeTab === "ast"}
					<div id="panel-ast" role="tabpanel" aria-labelledby="tab-ast">
						<p class="mb-3 text-xs text-muted-foreground">Parsed table declarations and fields, in source order.</p>
										{#if isParsed()}
											<pre class="whitespace-pre-wrap break-words rounded-xl bg-muted/40 p-4 font-mono text-xs leading-relaxed text-foreground">{json(parserResult?.status === "parsed" ? parserResult.tables : [])}</pre>
						{:else if parserResult?.status === "invalid"}
							<pre class="whitespace-pre-wrap break-words rounded-xl bg-amber-500/10 p-4 font-mono text-xs leading-relaxed text-amber-900 dark:text-amber-100">{json(parserResult.problems)}</pre>
						{:else}
							<p class="rounded-xl border border-border bg-muted/30 p-4 text-sm text-muted-foreground">The AST is unavailable until syntax is valid.</p>
						{/if}
					</div>
				{/if}
			</div>
		</article>
	</section>
</main>
