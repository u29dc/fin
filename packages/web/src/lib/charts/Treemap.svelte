<script lang="ts">
	import { onMount } from 'svelte';
	import { echarts, createTreemapOption, type ColorScheme, type TreemapDataItem } from './echarts';

	type Props = {
		data: TreemapDataItem[];
		colorScheme?: ColorScheme;
	};

	const { data, colorScheme = 'dark' }: Props = $props();

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

		const option = createTreemapOption(data, colorScheme);
		chart.setOption(option, true);
	}

	onMount(() => {
		return initChart();
	});

	$effect(() => {
		// Re-render when data or colorScheme changes
		data;
		colorScheme;
		updateChart();
	});
</script>

<div bind:this={container} class="w-full h-full min-h-[400px]"></div>
