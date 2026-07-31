<script lang="ts">
	import { onMount } from 'svelte';
	import { ensureReady, searchGaiji } from '$lib/wasm';

	let query = $state('');
	let ready = $state(false);
	let initializationError = $state<string>();

	const searchState = $derived.by(() => {
		if (initializationError) {
			return { status: 'error' as const, message: initializationError };
		}
		const q = query.trim();
		if (!ready || !q) return { status: 'loading' as const };
		return searchGaiji(q);
	});
	const matches = $derived(
		searchState.status === 'ready' ? searchState.matches.slice(0, 60) : []
	);

	onMount(async () => {
		try {
			await ensureReady();
			ready = true;
		} catch (error) {
			initializationError = error instanceof Error ? error.message : String(error);
		}
	});
</script>

<svelte:head>
	<title>aozora-proof — 外字検索</title>
</svelte:head>

<p class="mt-0 mb-4 max-w-[60ch] text-muted">
	外字注記の説明文から文字を探します。注記に使う文字とそのコードポイントを確認できます。
</p>

<section>
	<h2 class="mb-2 text-base tracking-[0.08em] text-muted uppercase">外字検索</h2>
	<input
		bind:value={query}
		type="search"
		placeholder="注記の説明文を検索（例: 尓－小）"
		class="w-full rounded-[10px] border border-line bg-white px-[0.9rem] py-[0.7rem] text-base focus:border-transparent focus:outline-2 focus:outline-accent"
	/>

	<ul class="mt-4">
		{#if searchState.status === 'error'}
			<li class="py-2 text-error">外字検索に失敗しました: {searchState.message}</li>
		{:else if query.trim() && searchState.status === 'loading'}
			<li class="py-2 text-muted">外字辞書を読み込んでいます…</li>
		{:else if query.trim() && matches.length === 0}
			<li class="py-2 text-muted">該当なし</li>
		{:else}
			{#each matches as m, i (i)}
				<li class="flex flex-wrap items-baseline gap-2.5 border-b border-line py-2">
					<span class="flex-none font-serif text-2xl">{m.char}</span>
					<code class="font-mono text-[0.8rem] text-muted">{m.codepoint}</code>
					<span class="flex-1">{m.description}</span>
				</li>
			{/each}
		{/if}
	</ul>
</section>
