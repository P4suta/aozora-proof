<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';

	let { children } = $props();

	const nav = [
		{ id: '/', href: resolve('/'), label: '校正' },
		{ id: '/gaiji', href: resolve('/gaiji'), label: '外字検索' }
	];
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

<div class="mx-auto max-w-[760px] px-5">
	<header class="pt-10 pb-4">
		<h1 class="text-3xl tracking-tight">aozora-proof</h1>
		<nav class="mt-3 mb-4 flex gap-4 text-[0.95rem]">
			{#each nav as item (item.id)}
				<a
					href={item.href}
					aria-current={page.route.id === item.id ? 'page' : undefined}
					class="border-b-2 border-transparent pb-0.5 text-muted no-underline hover:text-fg aria-[current=page]:border-accent aria-[current=page]:text-fg"
				>
					{item.label}
				</a>
			{/each}
		</nav>
	</header>

	<main class="pt-4 pb-12">
		{@render children()}
	</main>

	<footer class="mt-8 border-t border-line pt-6 pb-10 text-sm text-muted">
		<a class="text-accent" href="https://github.com/P4suta/aozora-proof">GitHub</a> · Apache-2.0 OR
		MIT
	</footer>
</div>
