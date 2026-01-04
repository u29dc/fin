<script lang="ts">
	import { onMount } from 'svelte';
	import { echarts, createSankeyOption, type ColorScheme, type SankeyNode, type SankeyLink } from './echarts';

	type Props = {
		nodes: SankeyNode[];
		links: SankeyLink[];
		colorScheme?: ColorScheme;
	};

	const { nodes, links, colorScheme = 'dark' }: Props = $props();

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

		const option = createSankeyOption(nodes, links, colorScheme);
		chart.setOption(option, true);
	}

	onMount(() => {
		return initChart();
	});

	$effect(() => {
		// Re-render when data or colorScheme changes
		nodes;
		links;
		colorScheme;
		updateChart();
	});
</script>

<div bind:this={container} class="w-full h-full"></div>
