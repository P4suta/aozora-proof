<script lang="ts">
	import { onMount } from 'svelte';
	import { check, ensureReady, ruleTitle } from '$lib/wasm';

	const SEV: Record<string, { order: number; label: string; badge: string }> = {
		error: { order: 0, label: 'エラー', badge: 'bg-error' },
		warning: { order: 1, label: '警告', badge: 'bg-warning' },
		note: { order: 2, label: '注意', badge: 'bg-note' }
	};

	let text = $state('①俱來髙');
	let ready = $state(false);
	let initializationError = $state<string>();

	const checkState = $derived.by(() => {
		if (initializationError) {
			return { status: 'error' as const, message: initializationError };
		}
		if (!ready) return { status: 'loading' as const };
		return check(text);
	});

	const findings = $derived.by(() => {
		if (checkState.status !== 'ready') return [];
		const data = [...checkState.findings];
		data.sort(
			(a, b) =>
				(SEV[a.severity]?.order ?? 9) - (SEV[b.severity]?.order ?? 9) ||
				a.utf8ByteSpan.start - b.utf8ByteSpan.start
		);
		return data.map((f) => ({
			...f,
			line: f.position.line,
			title: ruleTitle(f.code)
		}));
	});

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
	<title>aozora-proof — 青空文庫記法テキストの文字レベル校正</title>
</svelte:head>

<p class="mt-0 mb-4 max-w-[60ch] text-muted">
	青空文庫記法テキストの<strong class="font-semibold text-fg">文字レベル校正</strong>。 JIS X 0208
	適合・機種依存文字・旧字体/新字体・外字注記の指摘を、貼り付けと同時に表示します。
</p>

<section>
	<h2 class="mb-2 text-base tracking-[0.08em] text-muted uppercase">校正</h2>
	<textarea
		bind:value={text}
		spellcheck="false"
		placeholder="ここに青空文庫記法のテキストを貼り付け / 入力…"
		class="min-h-56 w-full resize-y rounded-[10px] border border-line bg-white px-4 py-[0.9rem] font-serif text-[1.05rem] focus:border-transparent focus:outline-2 focus:outline-accent"
	></textarea>

	<ul aria-live="polite" class="mt-4">
		{#if checkState.status === 'error'}
			<li class="py-2 text-error">校正処理に失敗しました: {checkState.message}</li>
		{:else if checkState.status === 'loading'}
			<li class="py-2 text-muted">校正エンジンを読み込んでいます…</li>
		{:else if findings.length === 0}
			<li class="py-2 text-muted">✓ 指摘はありません</li>
		{:else}
			{#each findings as f, i (i)}
				<li class="flex flex-wrap items-baseline gap-2.5 border-b border-line py-2">
					<span
						class="flex-none rounded-full px-2 py-[0.05rem] text-xs font-bold text-white {SEV[
							f.severity
						]?.badge ?? 'bg-muted'}"
					>
						{SEV[f.severity]?.label ?? f.severity}
					</span>
					<span class="flex-none text-[0.85rem] tabular-nums text-muted">L{f.line}</span>
					<span class="flex-1 basis-64">{f.canonicalMessage}</span>
					{#if f.title}
						<span
							class="flex-none rounded-md bg-line px-2 py-[0.05rem] text-[0.78rem] text-muted"
						>
							{f.title}
						</span>
					{/if}
					{#if f.fixAlternatives[0]}
						<span class="w-full text-[0.85rem] text-muted">
							↳ 提案: <span class="font-serif text-fg"
								>{f.fixAlternatives[0].label}</span
							>
						</span>
					{/if}
				</li>
			{/each}
		{/if}
	</ul>
</section>
