<script lang="ts">
	import '../app.css';
	import ViewportHeight from '$lib/ViewportHeight.svelte';

	let { children } = $props();
</script>

<ViewportHeight />

<!-- Skip link for keyboard/screen reader accessibility -->
<a
	href="#main"
	class="sr-only focus:not-sr-only focus:fixed focus:top-4 focus:left-4 focus:z-50 focus:px-4 focus:py-2 focus:bg-text focus:text-bg focus:rounded focus:text-sm focus:font-medium"
>
	Skip to main content
</a>

<div id="main">
	{@render children()}
</div>
