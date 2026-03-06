<script lang="ts">
	import { onMount } from 'svelte';

	onMount(() => {
		const supportsModernUnits = CSS.supports('height', '100svh');
		if (supportsModernUnits) return;

		const updateViewportHeight = () => {
			const height = window.visualViewport?.height ?? window.innerHeight;
			document.documentElement.style.setProperty('--vh', `${height * 0.01}px`);
		};

		const scheduleUpdate = () => {
			requestAnimationFrame(updateViewportHeight);
		};

		updateViewportHeight();

		window.visualViewport?.addEventListener('resize', scheduleUpdate);
		window.visualViewport?.addEventListener('scroll', scheduleUpdate);
		window.addEventListener('resize', scheduleUpdate);
		window.addEventListener('orientationchange', scheduleUpdate);

		return () => {
			window.visualViewport?.removeEventListener('resize', scheduleUpdate);
			window.visualViewport?.removeEventListener('scroll', scheduleUpdate);
			window.removeEventListener('resize', scheduleUpdate);
			window.removeEventListener('orientationchange', scheduleUpdate);
		};
	});
</script>
