<script lang="ts">
	import Monitor from '@lucide/svelte/icons/monitor';
	import Moon from '@lucide/svelte/icons/moon';
	import Sun from '@lucide/svelte/icons/sun';
	import { theme } from '$lib/theme.svelte';
</script>

<button
	type="button"
	onclick={() => theme.cycle()}
	class="p-2 border-t-2 border-transparent text-muted hover:text-text transition-colors"
	aria-label="Toggle theme"
	title={theme.setting === 'system'
		? 'System theme'
		: theme.setting === 'dark'
			? 'Dark theme'
			: 'Light theme'}
>
	{#if theme.setting === 'system'}
		<Monitor class="size-4" />
	{:else if theme.setting === 'dark'}
		<Moon class="size-4" />
	{:else}
		<Sun class="size-4" />
	{/if}
</button>
