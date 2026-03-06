<script lang="ts">
	import Monitor from '@lucide/svelte/icons/monitor';
	import Moon from '@lucide/svelte/icons/moon';
	import Sun from '@lucide/svelte/icons/sun';
	import { theme } from '$lib/theme.svelte';
</script>

<button
	type="button"
	onclick={() => theme.cycle()}
	class="min-h-[44px] min-w-[44px] flex items-center justify-center border-t-2 border-transparent text-muted hover:text-text"
	aria-label="Toggle theme, current: {theme.setting}"
>
	{#if theme.setting === 'system'}
		<Monitor class="size-4" aria-hidden="true" />
	{:else if theme.setting === 'dark'}
		<Moon class="size-4" aria-hidden="true" />
	{:else}
		<Sun class="size-4" aria-hidden="true" />
	{/if}
</button>
