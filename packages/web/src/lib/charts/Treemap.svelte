<script lang="ts">
	import { onMount } from 'svelte';
	import { echarts, createTreemapOption, type ColorScheme, type TreemapDataItem } from './echarts';

	type Props = {
		data: TreemapDataItem[];
		colorScheme?: ColorScheme;
		compact?: boolean;
	};

	const { data, colorScheme = 'dark', compact = false }: Props = $props();

	let container: HTMLDivElement;
	let chart: ReturnType<typeof echarts.init> | null = null;

	function initChart() {
		if (!container) return;

		chart = echarts.init(container, undefined, { renderer: 'canvas' });
		updateChart();

		const resizeObserver = new ResizeObserver(() => {
			chart?.resize();
		});
		resizeObserver.observe(container);

		return () => {
			resizeObserver.disconnect();
			chart?.dispose();
			chart = null;
		};
	}

	function updateChart() {
		if (!chart) return;

		const option = createTreemapOption(data, colorScheme, compact);
		chart.setOption(option, true);
	}

	onMount(() => {
		return initChart();
	});

	$effect(() => {
		// Re-render when data, colorScheme, or compact changes
		data;
		colorScheme;
		compact;
		updateChart();
	});
</script>

<div bind:this={container} class="w-full h-full"></div>
